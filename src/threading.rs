use std::{
    cell::Cell,
    ffi::c_char,
    io, mem,
    os::{raw::c_void, windows::raw::HANDLE},
    ptr::null_mut,
    sync::{
        Arc, Mutex,
        atomic::{self, AtomicU64, Ordering},
    },
};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use windows::{
    threadpoolapiset::{
        CloseThreadpoolWait, CreateThreadpoolWait, SetThreadpoolWait,
        WaitForThreadpoolWaitCallbacks,
    },
    winbase::WAIT_OBJECT_0,
    winnt::{self, PTP_CALLBACK_INSTANCE, PTP_WAIT, TP_WAIT_RESULT},
};
use windows_core::{
    ComObjectInner, ComObjectInterface, HRESULT, IUnknown, Interface, InterfaceRef, implement,
    interface,
};

use crate::{
    E_FAIL, E_NOTIMPL,
    results::{E_PENDING, S_OK},
};

#[repr(u32)]
enum XAsyncOp {
    Begin,
    DoWork,
    GetResult,
    Cancel,
    Cleanup,
}

#[repr(u32)]
enum XTaskQueueDispatchMode {
    Manual,
    ThreadPool,
    SerializedThreadPool,
    Immediate,
}
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
enum XTaskQueuePort {
    Work,
    Completion,
}

#[repr(C)]
struct XAsyncProviderData {
    async_: *mut XAsyncBlock,
    buffer_size: usize,
    buffer: *mut c_void,
    context: *mut c_void,
}

pub type XTaskQueueHandle = *mut c_void;
pub type XTaskQueuePortHandle = *mut c_void;
pub type XAsyncCompletionRoutine = unsafe extern "system" fn(*mut XAsyncBlock);
pub type XAsyncWork = unsafe extern "system" fn(*mut XAsyncBlock);
type XAsyncProvider =
    unsafe extern "system" fn(op: XAsyncOp, data: *const XAsyncProviderData) -> HRESULT;
pub type XTaskQueueCallback = unsafe extern "system" fn(context: *mut c_void, cancelled: bool);
pub type XTaskQueueTerminatedCallback = unsafe extern "system" fn(context: *mut c_void);
pub type XTaskQueueMonitorCallback =
    unsafe extern "system" fn(context: *mut c_void, queue: XTaskQueueHandle, port: XTaskQueuePort);
pub type XTaskQueueRegistrationToken = u64;

const XASYNC_INT_MAGIC: u32 = 0x5853594e; // "XSYN"
const XASYNC_STATE_MAGIC: u32 = 0x5853594f; // "XSYO"

#[repr(C)]
#[derive(Copy, Clone)]
pub struct XAsyncBlock {
    pub queue: XTaskQueueHandle,
    pub context: *mut c_void,
    pub callback: Option<XAsyncCompletionRoutine>,
    pub internal: [u8; size_of::<*mut c_void>() * 4],
}

struct XAsyncState {
    magic: u32,
    lock: atomic::AtomicBool,
    result_size: usize,
    queue: XTaskQueueHandle,
    context: *mut c_void,
    callback: Option<XAsyncCompletionRoutine>,
    ref_count: atomic::AtomicI32,
    user_block: *mut XAsyncBlock,
    async_block: XAsyncBlock,
    provider_data: XAsyncProviderData,
    async_provider: XAsyncProvider,
}

impl XAsyncState {
    fn new(
        queue: XTaskQueueHandle,
        context: *mut c_void,
        callback: Option<XAsyncCompletionRoutine>,
        async_block: *mut XAsyncBlock,
        async_provider: XAsyncProvider,
    ) -> Self {
        let user_internal = unsafe { &*async_block }.get_internal_raw();
        user_internal.magic = XASYNC_INT_MAGIC;
        user_internal.state = null_mut();
        user_internal.result = S_OK;
        user_internal.lock = atomic::AtomicBool::new(true);
        let provider_block = unsafe { *async_block };
        let internal = provider_block.get_internal_raw();
        XAsyncState {
            magic: XASYNC_STATE_MAGIC,
            lock: atomic::AtomicBool::new(false),
            result_size: 0,
            queue,
            context,
            callback,
            ref_count: atomic::AtomicI32::new(1),
            user_block: async_block,
            async_block: provider_block,
            async_provider: async_provider,
            provider_data: XAsyncProviderData {
                async_: async_block,
                buffer_size: 0,
                buffer: null_mut(),
                context,
            },
        }
    }
}

#[repr(C)]
struct XAsyncInternal {
    magic: u32,
    lock: atomic::AtomicBool,
    state: *mut XAsyncState,
    result: HRESULT,
}

struct XAsyncStateRef {
    state: *mut XAsyncState,
}

impl XAsyncStateRef {
    fn new(state: *mut XAsyncState) -> Self {
        unsafe { (*state).ref_count.fetch_add(1, Ordering::Relaxed) };
        XAsyncStateRef { state }
    }
}

impl Drop for XAsyncStateRef {
    fn drop(&mut self) {
        unsafe {
            self.state.as_mut().map(|f| {
                if f.ref_count.fetch_sub(1, Ordering::Release) == 1 {
                    // Clean up the state if the reference count reaches zero
                    Box::from_raw(f);
                }
            })
        };
    }
}

impl XAsyncBlock {
    fn get_internal_raw(&self) -> &mut XAsyncInternal {
        assert!(size_of::<XAsyncInternal>() <= self.internal.len());
        unsafe { &mut *(self.internal.as_ptr() as *mut XAsyncInternal) }
    }
    fn get_internal(&self) -> Option<&mut XAsyncInternal> {
        let internal = self.get_internal_raw();
        if internal.magic != XASYNC_INT_MAGIC {
            return None;
        }
        Some(internal)
    }
    fn Lock(&self) -> Option<XAsyncStateRef> {
        let Some(internal) = self.get_internal() else {
            return None;
        };
        while internal
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }

        let state = internal.state;

        internal
            .lock
            .compare_exchange(true, false, Ordering::Release, Ordering::Relaxed)
            .unwrap();

        // Here the original code tries to aquire the lock of the internat state copy
        while unsafe { &(*state).lock }
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }

        Some(XAsyncStateRef::new(state))
    }
}

// pub unsafe fn x_task_queue_monitor_callback (self: &Self, context: *mut c_void, queue: XTaskQueueHandle, port: XTaskQueuePort);

#[interface("073b7dcb-1fcf-4030-94be-e3c9eb623428")]
pub unsafe trait IXAsync: IUnknown {
    // get status / wait for completion.
    pub unsafe fn x_async_get_status(self: &Self, async_block: *mut XAsyncBlock, wait: bool);
    // Access stored result size, maybe return an error once it is fetched
    pub unsafe fn x_async_get_result_size(
        self: &Self,
        async_block: *mut XAsyncBlock,
        buffer_size: *mut usize,
    );
    // Call cancel of the provider, everything else seems to be up to the receiver
    pub unsafe fn x_async_cancel(self: &Self, async_block: *mut XAsyncBlock);
    // Just a wrapper of x_async_begin with only work callback
    pub unsafe fn x_async_run(self: &Self, async_block: *mut XAsyncBlock, work: *mut XAsyncWork);
    // Calls begin of provider synchronously, may already be completed on return of immediate mode
    pub unsafe fn x_async_begin(
        self: &Self,
        async_block: *mut XAsyncBlock,
        context: *mut c_void,
        identity: *mut c_void,
        identity_name: *const c_char,
        provider: Option<XAsyncProvider>,
    );
    // No clue
    pub unsafe fn ___1(self: &Self);
    // Wrapper of x_task_queue_submit_delayed_callback that invokes provider with DoWork
    pub unsafe fn x_async_schedule(self: &Self, async_block: *mut XAsyncBlock, delay_in_ms: u32);
    // First one wins and set the result code and payload
    pub unsafe fn x_async_complete(
        self: &Self,
        async_block: *mut XAsyncBlock,
        result: HRESULT,
        required_buffer_size: usize,
    );
    // Read the result, after success deallocate the state and return errors
    pub unsafe fn x_async_get_result(
        self: &Self,
        async_block: *mut XAsyncBlock,
        identity: *const c_void,
        buffer_size: usize,
        buffer: *mut c_void,
        buffer_used: *mut usize,
    );
    // create two task queue ports
    pub unsafe fn x_task_queue_create(
        self: &Self,
        work_dispatch_mode: XTaskQueueDispatchMode,
        completion_dispatch_mode: XTaskQueueDispatchMode,
        queue: *mut XTaskQueueHandle,
    );
    // reuses a port of another task_queue, so those needs to be arc handles internally
    pub unsafe fn x_task_queue_create_composite(
        self: &Self,
        work_port: XTaskQueuePortHandle,
        completion_port: XTaskQueuePortHandle,
        queue: *mut XTaskQueueHandle,
    ) -> HRESULT;
    // weak access to queue port, which is owned by this queue
    pub unsafe fn x_task_queue_get_port(
        self: &Self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        port_handle: *mut XTaskQueuePortHandle,
    );
    pub unsafe fn x_task_queue_duplicate_handle(
        self: &Self,
        queue_handle: XTaskQueueHandle,
        duplicated_handle: *mut XTaskQueueHandle,
    ) -> HRESULT;
    // Manual Queue Dispatching
    pub unsafe fn x_task_queue_dispatch(
        self: &Self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        timeout_in_ms: u32,
    );
    pub unsafe fn x_task_queue_close_handle(self: &Self, queue: XTaskQueueHandle);
    // Submit work to the queue port
    // Notifies callbacks of x_task_queue_register_monitor
    pub unsafe fn x_task_queue_submit_callback(
        self: &Self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT;
    pub unsafe fn x_task_queue_submit_delayed_callback(
        self: &Self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        delay_ms: u32,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT;
    // Who uses this? win32 only but supported by all gdk platforms
    pub unsafe fn x_task_queue_register_waiter(
        self: &Self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        wait_handle: HANDLE,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
        token: *mut XTaskQueueRegistrationToken,
    );
    pub unsafe fn x_task_queue_unregister_waiter(
        &self,
        queue: XTaskQueueHandle,
        token: XTaskQueueRegistrationToken,
    );
    pub unsafe fn x_task_queue_terminate(
        self: &Self,
        queue: XTaskQueueHandle,
        wait: bool,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueTerminatedCallback>,
    );
    // For manual queue notification for queued items
    pub unsafe fn x_task_queue_register_monitor(
        self: &Self,
        queue: XTaskQueueHandle,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueMonitorCallback>,
        token: *mut XTaskQueueRegistrationToken,
    );
    pub unsafe fn x_task_queue_unregister_monitor(
        self: &Self,
        queue: XTaskQueueHandle,
        token: XTaskQueueRegistrationToken,
    );
    // global static reference, e.g. store it in the global com object?
    pub unsafe fn x_task_queue_get_current_process_task_queue(
        self: &Self,
        queue: *mut XTaskQueueHandle,
    ) -> HRESULT;
    pub unsafe fn x_task_queue_set_current_process_task_queue(
        self: &mut Self,
        queue: XTaskQueueHandle,
    ) -> HRESULT;
    // The game forbids functions that are slow
    pub unsafe fn x_thread_set_time_sensitive(self: &Self, is_time_sensitive_thread: bool);
    // Private stuff of the gdk runtime, using thread locals etc.
    // No clue
    pub unsafe fn ___2(self: &Self);
    pub unsafe fn x_thread_assert_not_time_sensitive(self: &Self);
    pub unsafe fn x_thread_is_time_sensitive(self: &Self) -> bool;
}

unsafe extern "system" fn wait_callback(
    _instance: PTP_CALLBACK_INSTANCE,
    context: *mut c_void,
    wait: PTP_WAIT,
    result: TP_WAIT_RESULT,
) {
    if result.0 != WAIT_OBJECT_0 || context.is_null() {
        return;
    }

    let wctx = unsafe { &*(context.cast::<WaitCallbackContext>()) };
    // register the wait again, because it is a one-shot callback
    unsafe { SetThreadpoolWait(wait, Some(winnt::HANDLE(wctx.wait_handle)), None) };

    unsafe {
        wctx.port.submit_callback(
            wctx.tracker.clone(),
            wctx.context as *mut c_void,
            wctx.callback,
        )
    };
}

std::thread_local! {
    static IS_TIME_SENSITIVE: Cell<bool> = Cell::new(false);
}

#[implement(IXAsync)]
pub struct XAsync {
    process_queue: Mutex<XTaskQueueHandle>,
    runtime: tokio::runtime::Runtime,
}

#[interface("073b7dcb-1fcf-4030-94be-e3c9eb623428")]
unsafe trait ITaskQueue: IUnknown {
    unsafe fn get_handle(&self) -> XTaskQueueHandle;
    unsafe fn submit_delayed_callback(
        &self,
        port: XTaskQueuePort,
        delay_ms: u32,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT;
    unsafe fn dispatch(&self, port: XTaskQueuePort, timeout_in_ms: u32);
    unsafe fn get_port_handle(&self, port: XTaskQueuePort) -> XTaskQueuePortHandle;
    unsafe fn terminate(
        &self,
        wait: bool,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueTerminatedCallback>,
    );
    unsafe fn register_monitor(
        &self,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueMonitorCallback>,
        token: *mut XTaskQueueRegistrationToken,
    );
    unsafe fn unregister_monitor(&self, token: XTaskQueueRegistrationToken);
    unsafe fn register_waiter(
        &self,
        port: XTaskQueuePort,
        wait_handle: HANDLE,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
        token: *mut XTaskQueueRegistrationToken,
    );
    unsafe fn unregister_waiter(&self, queue: XTaskQueueHandle, token: XTaskQueueRegistrationToken);
}

#[interface("073b7dcb-1fcf-4030-94be-e3c9eb623428")]
unsafe trait ITaskQueuePort: IUnknown {
    unsafe fn get_handle(&self) -> XTaskQueuePortHandle;
    unsafe fn submit_callback(
        &self,
        tracker: tokio_util::task::TaskTracker,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT;
    // TODO put this into Queue itself and call submit_delayed_callback when ready?
    // unsafe fn register_waiter(&self,wait_handle: HANDLE,callback_context: *mut c_void,callback: Option<XTaskQueueCallback>,token: *mut XTaskQueueRegistrationToken);
    // unsafe fn unregister_waiter(&self, queue: XTaskQueueHandle, token: XTaskQueueRegistrationToken);
    unsafe fn dispatch(&self, timeout_in_ms: u32);
}

#[implement(ITaskQueuePort)]
struct TaskQueuePort {
    runtime: tokio::runtime::Runtime,
}

impl TaskQueuePort {
    fn new_thread_pool() -> io::Result<ITaskQueuePort> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        Ok(Self { runtime }.into())
    }
    fn new_serialized_thread_pool() -> io::Result<ITaskQueuePort> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()?;
        Ok(Self { runtime }.into())
    }
}

impl ITaskQueuePort_Impl for TaskQueuePort_Impl {
    unsafe fn get_handle(&self) -> XTaskQueuePortHandle {
        let unk: InterfaceRef<ITaskQueuePort> = self.as_interface_ref();
        unk.as_raw()
    }

    unsafe fn submit_callback(
        &self,
        tracker: tokio_util::task::TaskTracker,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT {
        let ctx = callback_context as u64;
        tracker.clone().spawn_on(
            async move {
                if let Some(cb) = callback.as_ref() {
                    unsafe { cb(ctx as *mut c_void, tracker.is_closed()) };
                }
            },
            self.runtime.handle(),
        );
        S_OK
    }

    unsafe fn dispatch(&self, _timeout_in_ms: u32) {}
}

#[implement(ITaskQueuePort)]
struct ImmediateTaskQueuePort {}

impl ImmediateTaskQueuePort {
    fn new() -> ITaskQueuePort {
        Self {}.into()
    }
}

impl ITaskQueuePort_Impl for ImmediateTaskQueuePort_Impl {
    unsafe fn get_handle(&self) -> XTaskQueuePortHandle {
        // &self.this as *const _ as XTaskQueuePortHandle
        let unk: InterfaceRef<ITaskQueuePort> = self.as_interface_ref();
        unk.as_raw()
    }

    unsafe fn submit_callback(
        &self,
        tracker: tokio_util::task::TaskTracker,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT {
        let token = tracker.token();
        callback.map(|f| f(callback_context, tracker.is_closed()));
        mem::drop(token);
        S_OK
    }

    unsafe fn dispatch(&self, timeout_in_ms: u32) {}
}

struct QueueEntry {
    token: tokio_util::task::task_tracker::TaskTrackerToken,
    callback: Option<XTaskQueueCallback>,
    context: u64,
}

#[implement(ITaskQueuePort)]
struct ManualTaskQueuePort {
    tx: std::sync::mpsc::Sender<QueueEntry>,
    rx: std::sync::mpsc::Receiver<QueueEntry>,
}

impl ManualTaskQueuePort {
    fn new() -> ITaskQueuePort {
        let (tx, rx) = std::sync::mpsc::channel();
        Self { tx, rx }.into()
    }
}

impl ITaskQueuePort_Impl for ManualTaskQueuePort_Impl {
    unsafe fn get_handle(&self) -> XTaskQueuePortHandle {
        let unk: InterfaceRef<ITaskQueuePort> = self.as_interface_ref();
        unk.as_raw()
    }

    unsafe fn submit_callback(
        &self,
        tracker: tokio_util::task::TaskTracker,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT {
        // Call monitior somewhere before or after this
        self.tx
            .send(QueueEntry {
                callback,
                context: callback_context as u64,
                token: tracker.token(),
            })
            .map_err(|_| E_FAIL)
            .unwrap();
        S_OK
    }

    unsafe fn dispatch(&self, timeout_in_ms: u32) {
        self.rx
            .recv_timeout(std::time::Duration::from_millis(timeout_in_ms as u64))
            .map(|entry| {
                entry.callback.map(|f| unsafe {
                    f(
                        entry.context as *mut c_void,
                        entry.token.task_tracker().is_closed(),
                    )
                });
            })
            .ok();
    }
}

#[implement(ITaskQueue)]
struct TaskQueue {
    work: ITaskQueuePort,
    completion: ITaskQueuePort,
    tracker: tokio_util::task::TaskTracker,
    handle: tokio::runtime::Handle,
    close_token: CancellationToken,
    // TODO lockless handles + generation
    monitor_handles: Arc<Mutex<Vec<(XTaskQueueRegistrationToken, XTaskQueueMonitorCallback, u64)>>>,
    next_handle: AtomicU64,
    waiter_handles: Arc<
        Mutex<
            Vec<(
                XTaskQueueRegistrationToken,
                *mut winnt::TP_WAIT,
                Box<WaitCallbackContext>,
            )>,
        >,
    >,
    next_waiter_handle: AtomicU64,
}

impl TaskQueue {
    fn new(
        handle: tokio::runtime::Handle,
        work: ITaskQueuePort,
        completion: ITaskQueuePort,
    ) -> ITaskQueue {
        Self {
            work,
            completion,
            tracker: tokio_util::task::TaskTracker::new(),
            handle,
            close_token: CancellationToken::new(),
            monitor_handles: Arc::new(Mutex::new(Vec::new())),
            next_handle: AtomicU64::new(0),
            waiter_handles: Arc::new(Mutex::new(Vec::new())),
            next_waiter_handle: AtomicU64::new(0),
        }
        .into()
    }

    fn get_port(&self, port: XTaskQueuePort) -> &ITaskQueuePort {
        match port {
            XTaskQueuePort::Work => &self.work,
            XTaskQueuePort::Completion => &self.completion,
        }
    }
}

struct WaitCallbackContext {
    port: ITaskQueuePort,
    wait_handle: HANDLE,
    callback: Option<XTaskQueueCallback>,
    context: u64,
    tracker: tokio_util::task::TaskTracker,
}

impl ITaskQueue_Impl for TaskQueue_Impl {
    unsafe fn get_handle(&self) -> XTaskQueueHandle {
        let unk: InterfaceRef<ITaskQueue> = self.as_interface_ref();
        unk.as_raw()
    }

    unsafe fn submit_delayed_callback(
        &self,
        port: XTaskQueuePort,
        delay_ms: u32,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT {
        let tracker = self.tracker.clone();
        let oport = self.get_port(port);
        if delay_ms == 0 {
            self.monitor_handles
                .lock()
                .unwrap()
                .iter()
                .for_each(|(_, callback, context)| {
                    unsafe { callback(*context as *mut c_void, self.get_handle(), port) };
                });
            unsafe { oport.submit_callback(tracker, callback_context, callback) }
        } else {
            let callback_context = callback_context as u64;
            let oport = oport.clone().into_raw() as u64;
            let cancel_token = self.close_token.clone();
            let monitor_handles = self.monitor_handles.clone();
            let handle = unsafe { self.get_handle() } as u64;
            self.tracker.spawn_on(
                async move {
                    cancel_token
                        .run_until_cancelled(tokio::time::sleep(std::time::Duration::from_millis(
                            delay_ms as u64,
                        )))
                        .await;
                    monitor_handles
                        .lock()
                        .unwrap()
                        .iter()
                        .for_each(|(_, callback, context)| {
                            unsafe {
                                callback(*context as *mut c_void, handle as XTaskQueueHandle, port)
                            };
                        });
                    unsafe {
                        ITaskQueuePort::from_raw(oport as *mut c_void).submit_callback(
                            tracker,
                            callback_context as *mut c_void,
                            callback,
                        )
                    };
                },
                &self.handle,
            );
            S_OK
        }
    }

    unsafe fn dispatch(&self, port: XTaskQueuePort, timeout_in_ms: u32) {
        unsafe { self.get_port(port).dispatch(timeout_in_ms) }
    }

    unsafe fn get_port_handle(&self, port: XTaskQueuePort) -> XTaskQueuePortHandle {
        unsafe { self.get_port(port).get_handle() }
    }

    unsafe fn terminate(
        &self,
        wait: bool,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueTerminatedCallback>,
    ) {
        let callback_context = callback_context as u64;
        let tracker = self.tracker.clone();
        let future = async move {
            tracker.wait().await;
            if let Some(cb) = callback.as_ref() {
                unsafe { cb(callback_context as *mut c_void) };
            }
        };
        self.tracker.close();
        self.close_token.cancel();
        if wait {
            self.handle.block_on(future);
        } else {
            self.handle.spawn(future);
        }
    }

    unsafe fn register_monitor(
        &self,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueMonitorCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) {
        let Some(callback) = callback else {
            return;
        };
        let mut monitor_handles = self.monitor_handles.lock().unwrap();
        let handle = self.next_handle.fetch_add(1, Ordering::SeqCst);
        monitor_handles.push((handle, callback, callback_context as u64));
        if !token.is_null() {
            unsafe {
                *token = handle;
            }
        }
    }

    unsafe fn unregister_monitor(&self, token: XTaskQueueRegistrationToken) {
        let mut monitor_handles = self.monitor_handles.lock().unwrap();
        if let Some(pos) = monitor_handles.iter().position(|(h, _, _)| *h == token) {
            monitor_handles.remove(pos);
        }
    }

    unsafe fn register_waiter(
        &self,
        port: XTaskQueuePort,
        wait_handle: HANDLE,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) {
        let mut waiter_handles = self.waiter_handles.lock().unwrap();

        let oport = self.get_port(port);
        let wctx = Box::new(WaitCallbackContext {
            port: oport.clone(),
            wait_handle,
            callback,
            context: callback_context as u64,
            tracker: self.tracker.clone(),
        });
        let raw = Box::into_raw(wctx);
        let wctx = unsafe { Box::from_raw(raw) };

        let wait =
            unsafe { CreateThreadpoolWait(Some(wait_callback), Some(raw as *mut c_void), None) };
        unsafe { SetThreadpoolWait(wait, Some(winnt::HANDLE(wait_handle)), None) };
        let handle = self.next_waiter_handle.fetch_add(1, Ordering::SeqCst);
        waiter_handles.push((handle, wait, wctx));
        if !token.is_null() {
            unsafe {
                *token = handle;
            }
        }
    }

    unsafe fn unregister_waiter(
        &self,
        queue: XTaskQueueHandle,
        token: XTaskQueueRegistrationToken,
    ) {
        let mut waiter_handles = self.waiter_handles.lock().unwrap();
        if let Some(pos) = waiter_handles.iter().position(|h| h.0 == token) {
            let (_, waiter, _) = waiter_handles.remove(pos);
            unsafe { SetThreadpoolWait(waiter, None, None) };
            unsafe { WaitForThreadpoolWaitCallbacks(waiter, true) };
            unsafe { CloseThreadpoolWait(waiter) };
        }
    }
}

impl IXAsync_Impl for XAsync_Impl {
    unsafe fn x_async_get_status(&self, async_block: *mut XAsyncBlock, wait: bool) {
        let (tx, rx) = mpsc::channel(1);
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap()
            .block_on(async {
                // Simulate some async work
                tx.send(0).await.unwrap();

                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            });
    }

    unsafe fn x_async_get_result_size(
        &self,
        async_block: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) {
        todo!()
    }

    unsafe fn x_async_cancel(&self, async_block: *mut XAsyncBlock) {
        todo!()
    }

    unsafe fn x_async_run(&self, async_block: *mut XAsyncBlock, work: *mut XAsyncWork) {
        todo!()
    }

    unsafe fn x_async_begin(
        &self,
        async_block: *mut XAsyncBlock,
        context: *mut c_void,
        identity: *mut c_void,
        identity_name: *const c_char,
        provider: Option<XAsyncProvider>,
    ) {
        let blk = unsafe { &mut *async_block };
        // blk.
        let state_ref = blk.Lock();

        provider.map(|provider| {
            let data = XAsyncProviderData {
                async_: async_block,
                buffer_size: 0,
                buffer: null_mut(),
                context,
            };
            unsafe { provider(XAsyncOp::Begin, &data) };
        });
        todo!()
    }

    unsafe fn ___1(&self) {
        todo!()
    }

    unsafe fn x_async_schedule(&self, async_block: *mut XAsyncBlock, delay_in_ms: u32) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.spawn(async {});
        todo!()
    }

    unsafe fn x_async_complete(
        &self,
        async_block: *mut XAsyncBlock,
        result: HRESULT,
        required_buffer_size: usize,
    ) {
        todo!()
    }

    unsafe fn x_async_get_result(
        &self,
        async_block: *mut XAsyncBlock,
        identity: *const c_void,
        buffer_size: usize,
        buffer: *mut c_void,
        buffer_used: *mut usize,
    ) {
        todo!()
    }

    unsafe fn x_task_queue_create(
        &self,
        work_dispatch_mode: XTaskQueueDispatchMode,
        completion_dispatch_mode: XTaskQueueDispatchMode,
        queue: *mut XTaskQueueHandle,
    ) {
        let work: ITaskQueuePort = match work_dispatch_mode {
            XTaskQueueDispatchMode::Manual => ManualTaskQueuePort::new(),
            XTaskQueueDispatchMode::ThreadPool => TaskQueuePort::new_thread_pool().unwrap(),
            XTaskQueueDispatchMode::SerializedThreadPool => {
                TaskQueuePort::new_serialized_thread_pool().unwrap()
            }
            XTaskQueueDispatchMode::Immediate => ImmediateTaskQueuePort::new(),
        };
        let completion: ITaskQueuePort = match completion_dispatch_mode {
            XTaskQueueDispatchMode::Manual => ManualTaskQueuePort::new(),
            XTaskQueueDispatchMode::ThreadPool => TaskQueuePort::new_thread_pool().unwrap(),
            XTaskQueueDispatchMode::SerializedThreadPool => {
                TaskQueuePort::new_serialized_thread_pool().unwrap()
            }
            XTaskQueueDispatchMode::Immediate => ImmediateTaskQueuePort::new(),
        };
        let task_queue: ITaskQueue =
            TaskQueue::new(self.runtime.handle().clone(), work, completion);
        unsafe {
            *queue = task_queue.get_handle();
        }
        mem::forget(task_queue);
    }

    unsafe fn x_task_queue_create_composite(
        &self,
        work_port: XTaskQueuePortHandle,
        completion_port: XTaskQueuePortHandle,
        queue: *mut XTaskQueueHandle,
    ) -> HRESULT {
        let work = unsafe { ITaskQueuePort::from_raw_borrowed(&work_port) };
        let completion = unsafe { ITaskQueuePort::from_raw_borrowed(&completion_port) };
        let (Some(work), Some(completion)) = (work, completion) else {
            unsafe {
                *queue = null_mut();
            }
            return E_FAIL;
        };
        let task_queue: ITaskQueue = TaskQueue::new(
            self.runtime.handle().clone(),
            work.clone(),
            completion.clone(),
        );
        unsafe {
            *queue = task_queue.get_handle();
        }
        mem::forget(task_queue);
        S_OK
    }

    unsafe fn x_task_queue_get_port(
        &self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        port_handle: *mut XTaskQueuePortHandle,
    ) {
        let queue = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        let Some(queue) = queue else {
            unsafe {
                *port_handle = null_mut();
            }
            return;
        };

        unsafe {
            *port_handle = queue.get_port_handle(port);
        }
    }

    unsafe fn x_task_queue_duplicate_handle(
        &self,
        queue_handle: XTaskQueueHandle,
        duplicated_handle: *mut XTaskQueueHandle,
    ) -> HRESULT {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue_handle) };
        let Some(handle) = handle.map(|f| {
            let clone = f.clone();
            mem::forget(clone);
            unsafe { f.get_handle() }
        }) else {
            return E_FAIL;
        };
        unsafe { *duplicated_handle = handle };
        S_OK
    }

    unsafe fn x_task_queue_dispatch(
        &self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        timeout_in_ms: u32,
    ) {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| f.dispatch(port, timeout_in_ms));
    }

    unsafe fn x_task_queue_close_handle(&self, queue: XTaskQueueHandle) {
        unsafe {
            ITaskQueue::from_raw(queue);
        }
    }

    unsafe fn x_task_queue_submit_callback(
        &self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT {
        unsafe {
            self.x_task_queue_submit_delayed_callback(queue, port, 0, callback_context, callback)
        }
    }

    unsafe fn x_task_queue_submit_delayed_callback(
        &self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        delay_ms: u32,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
    ) -> HRESULT {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| f.submit_delayed_callback(port, delay_ms, callback_context, callback));

        S_OK
    }

    unsafe fn x_task_queue_register_waiter(
        &self,
        queue: XTaskQueueHandle,
        port: XTaskQueuePort,
        wait_handle: HANDLE,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| unsafe {
            f.register_waiter(port, wait_handle, callback_context, callback, token)
        });
    }

    unsafe fn x_task_queue_unregister_waiter(
        &self,
        queue: XTaskQueueHandle,
        token: XTaskQueueRegistrationToken,
    ) {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| unsafe { f.unregister_waiter(queue, token) });
    }

    unsafe fn x_task_queue_terminate(
        &self,
        queue: XTaskQueueHandle,
        wait: bool,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueTerminatedCallback>,
    ) {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| unsafe { f.terminate(wait, callback_context, callback) });
    }

    unsafe fn x_task_queue_register_monitor(
        &self,
        queue: XTaskQueueHandle,
        callback_context: *mut c_void,
        callback: Option<XTaskQueueMonitorCallback>,
        token: *mut XTaskQueueRegistrationToken,
    ) {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| f.register_monitor(callback_context, callback, token));
    }

    unsafe fn x_task_queue_unregister_monitor(
        &self,
        queue: XTaskQueueHandle,
        token: XTaskQueueRegistrationToken,
    ) {
        let handle = unsafe { ITaskQueue::from_raw_borrowed(&queue) };
        handle.map(|f| f.unregister_monitor(token));
    }

    unsafe fn x_task_queue_get_current_process_task_queue(
        &self,
        queue: *mut XTaskQueueHandle,
    ) -> HRESULT {
        let a = self.process_queue.lock();
        if a.is_err() {
            return E_FAIL;
        }
        unsafe { *queue = *a.unwrap() };
        S_OK
    }

    unsafe fn x_task_queue_set_current_process_task_queue(
        &self,
        queue: XTaskQueueHandle,
    ) -> HRESULT {
        let mut lck = self.process_queue.lock().unwrap();
        *lck = queue;
        S_OK
    }

    unsafe fn x_thread_set_time_sensitive(&self, is_time_sensitive_thread: bool) {
        IS_TIME_SENSITIVE.with(|is_time_sensitive| {
            is_time_sensitive.set(is_time_sensitive_thread);
        });
    }

    unsafe fn x_thread_assert_not_time_sensitive(&self) {
        assert!(!IS_TIME_SENSITIVE.with(|is_time_sensitive| is_time_sensitive.get()));
    }

    unsafe fn x_thread_is_time_sensitive(&self) -> bool {
        IS_TIME_SENSITIVE.with(|is_time_sensitive| is_time_sensitive.get())
    }

    unsafe fn ___2(&self) {
        todo!()
    }
}

unsafe extern "system" fn callback(ctx: *mut c_void, cancel: bool) {
    println!(
        "Callback called with context: {:?}, cancel: {} {:?}",
        ctx,
        cancel,
        std::thread::current().id()
    );
}

#[test]
fn test_x_async() {
    let xasync: IXAsync = XAsync {
        process_queue: Mutex::new(null_mut()),
        runtime: tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap(),
    }
    .into();
    let mut queue: XTaskQueueHandle = null_mut();
    let mut queue2: XTaskQueueHandle = null_mut();
    let mut port_handle: XTaskQueuePortHandle = null_mut();
    unsafe {
        println!("Creating task queue... {:?}", std::thread::current().id());
        xasync.x_task_queue_create(
            XTaskQueueDispatchMode::SerializedThreadPool,
            XTaskQueueDispatchMode::Immediate,
            &mut queue,
        );
        println!("Submitting callback to task queue... {}", queue as usize);
        xasync.x_task_queue_submit_delayed_callback(
            queue,
            XTaskQueuePort::Work,
            0,
            null_mut(),
            Some(callback),
        );
        println!("Submitting delayed callback to task queue...");
        xasync.x_task_queue_submit_callback(
            queue,
            XTaskQueuePort::Work,
            null_mut(),
            Some(callback),
        );
        xasync.x_task_queue_get_port(queue, XTaskQueuePort::Work, &mut port_handle);

        xasync.x_task_queue_create_composite(port_handle, port_handle, &mut queue2);

        xasync.x_task_queue_terminate(queue, true, null_mut(), None);

        xasync.x_task_queue_close_handle(queue);

        xasync.x_task_queue_submit_delayed_callback(
            queue2,
            XTaskQueuePort::Work,
            0,
            null_mut(),
            Some(callback),
        );
        xasync.x_task_queue_submit_delayed_callback(
            queue2,
            XTaskQueuePort::Work,
            1000,
            null_mut(),
            Some(callback),
        );

        xasync.x_task_queue_terminate(queue2, true, null_mut(), None);

        xasync.x_task_queue_close_handle(queue2);

        // std::thread::sleep(std::time::Duration::from_millis(100));
        println!("Test completed.");
    };
}

#[test]
fn test_x_async2() {
    let port = TaskQueuePort::new_thread_pool().unwrap();

    let handle = unsafe { port.get_handle() };
    let obj = unsafe { ITaskQueuePort::from_raw_borrowed(&handle) };
    let nh = unsafe { obj.unwrap().get_handle() };
}
