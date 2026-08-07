use std::{
    cell::Cell,
    ffi::c_char,
    io, mem,
    os::{raw::c_void, windows::raw::HANDLE},
    ptr::null_mut,
    sync::{
        Arc, Condvar, Mutex,
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
const XASYNC_INITIALIZE_MAGIC: u32 = 0x5853394e; // "XSIN"
const XASYNC_STATE_MAGIC: u32 = 0x5853594f; // "XSYO"

#[repr(C)]
#[derive(Copy, Clone)]
pub struct XAsyncBlock {
    pub queue: XTaskQueueHandle,
    pub context: *mut c_void,
    pub callback: Option<XAsyncCompletionRoutine>,
    pub internal: [u8; size_of::<*mut c_void>() * 4],
}

#[interface("073b7dcb-1fcf-5030-94be-e3c9eb623428")]
unsafe trait IXAsyncState: IUnknown {
    unsafe fn get_local_block(&self) -> *mut XAsyncBlock;
    unsafe fn get_user_block(&self) -> *mut XAsyncBlock;
    unsafe fn get_provider_data(&self) -> *mut XAsyncProviderData;
    unsafe fn get_result_size(&self) -> usize;
    unsafe fn set_result_size(&self, required_buffer_size: usize);
    unsafe fn get_provider(&self) -> XAsyncProvider;
    unsafe fn get_waiter(&self) -> Arc<(Mutex<usize>, Condvar)>;
}

#[implement(IXAsyncState)]
struct XAsyncState {
    local_block: XAsyncBlock,
    user_block: *mut XAsyncBlock,
    provider_data: XAsyncProviderData,
    provider: XAsyncProvider,
    waiter: Arc<(Mutex<usize>, Condvar)>,
}

impl IXAsyncState_Impl for XAsyncState_Impl {
    unsafe fn get_local_block(&self) -> *mut XAsyncBlock {
        &self.local_block as *const _ as *mut XAsyncBlock
    }

    unsafe fn get_user_block(&self) -> *mut XAsyncBlock {
        self.user_block
    }

    unsafe fn get_provider_data(&self) -> *mut XAsyncProviderData {
        &self.provider_data as *const _ as *mut XAsyncProviderData
    }

    unsafe fn get_result_size(&self) -> usize {
        self.waiter.0.lock().unwrap().clone()
    }

    unsafe fn get_provider(&self) -> XAsyncProvider {
        self.provider
    }

    unsafe fn get_waiter(&self) -> Arc<(Mutex<usize>, Condvar)> {
        self.waiter.clone()
    }

    unsafe fn set_result_size(&self, required_buffer_size: usize) {
        let mut res_size = self.waiter.0.lock().unwrap();
        *res_size = required_buffer_size;
        self.waiter.1.notify_all();
    }
}

#[repr(C)]
struct XAsyncInternal {
    state: atomic::AtomicPtr<c_void>,
    magic_result: atomic::AtomicU64,
}

impl XAsyncBlock {
    fn get_internal_raw(&self) -> &mut XAsyncInternal {
        assert!(size_of::<XAsyncInternal>() <= self.internal.len());
        unsafe { &mut *(self.internal.as_ptr() as *mut XAsyncInternal) }
    }
    fn get_state_ex(&self, detach: bool) -> (Option<IXAsyncState>, Option<HRESULT>) {
        let internal = self.get_internal_raw();
        let magic_result = internal.magic_result.fetch_or(0, Ordering::Acquire);
        if (magic_result >> 32) as u32 != XASYNC_INT_MAGIC {
            return (None, None);
        }

        let result = HRESULT((magic_result & 0xFFFFFFFF) as i32);
        let result = if result == E_PENDING {
            None
        } else {
            Some(result)
        };

        let state_ptr = if detach {
            internal.state.swap(null_mut(), Ordering::AcqRel)
        } else {
            internal.state.load(Ordering::Acquire)
        };
        if state_ptr.is_null() {
            return (None, result);
        }

        if detach {
            let state = unsafe { IXAsyncState::from_raw(state_ptr) };

            let provider_block = unsafe { &*state.get_local_block() };
            let state_ptr_2 = provider_block
                .get_internal_raw()
                .state
                .swap(null_mut(), Ordering::AcqRel);
            assert!(state_ptr_2 == state_ptr);
            mem::drop(unsafe { IXAsyncState::from_raw(state_ptr) });
            (Some(state), result)
        } else {
            let state = unsafe { IXAsyncState::from_raw_borrowed(&state_ptr) };

            (state.map(|s| s.clone()), result)
        }
    }
    fn get_state(&self) -> (Option<IXAsyncState>, Option<HRESULT>) {
        self.get_state_ex(false)
    }

    fn create_state(&self, context: *mut c_void, provider: XAsyncProvider) -> Option<IXAsyncState> {
        let internal = self.get_internal_raw();
        let mut magic_result = internal.magic_result.fetch_or(0, Ordering::Acquire);
        if (magic_result >> 32) as u32 == XASYNC_INT_MAGIC {
            return None;
        }
        loop {
            if (magic_result >> 32) as u32 != XASYNC_INITIALIZE_MAGIC {
                break;
            }
            magic_result = internal.magic_result.fetch_or(0, Ordering::Acquire);
        }

        if (magic_result >> 32) as u32 == XASYNC_INT_MAGIC {
            return None;
        }

        if internal
            .magic_result
            .compare_exchange(
                magic_result,
                (XASYNC_INITIALIZE_MAGIC as u64) << 32,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_err()
        {
            return None;
        }

        let state: IXAsyncState = XAsyncState {
            local_block: *self,
            user_block: self as *const _ as *mut XAsyncBlock,
            provider_data: XAsyncProviderData {
                async_: null_mut() as *mut XAsyncBlock,
                buffer_size: 0,
                buffer: null_mut(),
                context: context,
            },
            provider: provider,
            waiter: Arc::new((Mutex::new(0), Condvar::new())),
        }
        .into();

        internal
            .state
            .store(state.clone().into_raw(), Ordering::Release);

        let local_block = unsafe { &*state.get_local_block() };

        let local_internal = local_block.get_internal_raw();
        local_internal.magic_result.store(
            (XASYNC_INT_MAGIC as u64) << 32 | (E_PENDING.0 as u64),
            Ordering::Release,
        );
        local_internal
            .state
            .store(state.clone().into_raw(), Ordering::Release);

        let provider_data = unsafe { &mut *state.get_provider_data() };
        provider_data.async_ = unsafe { state.get_local_block() };
        // signal no garbage bytes are stored
        internal.magic_result.store(
            (XASYNC_INT_MAGIC as u64) << 32 | (E_PENDING.0 as u64),
            Ordering::Release,
        );

        Some(state)
    }
}

// pub unsafe fn x_task_queue_monitor_callback (self: &Self, context: *mut c_void, queue: XTaskQueueHandle, port: XTaskQueuePort);

#[interface("073b7dcb-1fcf-4030-94be-e3c9eb623428")]
pub unsafe trait IXAsync: IUnknown {
    // get status / wait for completion.
    pub unsafe fn x_async_get_status(
        self: &Self,
        async_block: *mut XAsyncBlock,
        wait: bool,
    ) -> HRESULT;
    // Access stored result size, maybe return an error once it is fetched
    pub unsafe fn x_async_get_result_size(
        self: &Self,
        async_block: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT;
    // Call cancel of the provider, everything else seems to be up to the receiver
    pub unsafe fn x_async_cancel(self: &Self, async_block: *mut XAsyncBlock) -> HRESULT;
    // Just a wrapper of x_async_begin with only work callback
    pub unsafe fn x_async_run(
        self: &Self,
        async_block: *mut XAsyncBlock,
        work: *mut XAsyncWork,
    ) -> HRESULT;
    // Calls begin of provider synchronously, may already be completed on return of immediate mode
    pub unsafe fn x_async_begin(
        self: &Self,
        async_block: *mut XAsyncBlock,
        context: *mut c_void,
        identity: *mut c_void,
        identity_name: *const c_char,
        provider: Option<XAsyncProvider>,
    ) -> HRESULT;
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
    ) -> HRESULT;
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
    unsafe fn unregister_waiter(&self, token: XTaskQueueRegistrationToken);
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

    unsafe fn unregister_waiter(&self, token: XTaskQueueRegistrationToken) {
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
    unsafe fn x_async_get_status(&self, async_block: *mut XAsyncBlock, wait: bool) -> HRESULT {
        let blk = unsafe { &mut *async_block };
        match blk.get_state() {
            (Some(state), hr) => {
                // Use state and hr as needed
                if wait {
                    let waiter = unsafe { state.get_waiter() };

                    let lck = waiter.0.lock().unwrap();
                    let _lck = waiter
                        .1
                        .wait_while(lck, |_| blk.get_state().1.is_none())
                        .unwrap();

                    blk.get_state().1.unwrap()
                } else {
                    hr.unwrap_or(E_PENDING)
                }
            }
            (None, Some(hr)) => hr,
            _ => E_FAIL,
        }
    }

    unsafe fn x_async_get_result_size(
        &self,
        async_block: *mut XAsyncBlock,
        buffer_size: *mut usize,
    ) -> HRESULT {
        let blk = unsafe { &mut *async_block };
        let (Some(state), _) = blk.get_state() else {
            return E_FAIL;
        };
        if buffer_size.is_null() {
            return E_FAIL;
        }
        unsafe {
            *buffer_size = state.get_result_size();
        }
        S_OK
    }

    unsafe fn x_async_cancel(&self, async_block: *mut XAsyncBlock) -> HRESULT {
        let blk = unsafe { &mut *async_block };
        let (Some(state), _) = blk.get_state() else {
            return E_FAIL;
        };
        let provider_data = unsafe { &*state.get_provider_data() };
        let provider = unsafe { state.get_provider() };
        unsafe { provider(XAsyncOp::Cancel, provider_data) }
    }

    unsafe fn x_async_run(&self, async_block: *mut XAsyncBlock, work: *mut XAsyncWork) -> HRESULT {
        todo!()
    }

    unsafe fn x_async_begin(
        &self,
        async_block: *mut XAsyncBlock,
        context: *mut c_void,
        _identity: *mut c_void,
        _identity_name: *const c_char,
        provider: Option<XAsyncProvider>,
    ) -> HRESULT {
        let blk = unsafe { &mut *async_block };
        let Some(provider) = provider else {
            return E_FAIL;
        };
        let Some(state) = blk.create_state(context, provider) else {
            return E_FAIL;
        };

        let provider_data = unsafe { &*state.get_provider_data() };
        unsafe { provider(XAsyncOp::Begin, provider_data) }
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
        let blk = unsafe { &mut *async_block };
        // required_buffer_size == 0 => cleanup state as no result is expected, otherwise the state is kept until x_async_get_result is called
        let (Some(state), _) = blk.get_state_ex(required_buffer_size == 0) else {
            return;
        };
        if result == E_PENDING {
            return;
        }
        let user_block = unsafe { &*state.get_user_block() };
        if user_block
            .get_internal_raw()
            .magic_result
            .compare_exchange(
                (XASYNC_INT_MAGIC as u64) << 32 | (E_PENDING.0 as u64),
                (XASYNC_INT_MAGIC as u64) << 32 | (result.0 as u64),
                Ordering::Release,
                Ordering::Relaxed,
            )
            .is_ok()
        {
            unsafe { state.set_result_size(required_buffer_size) };
        }
    }

    unsafe fn x_async_get_result(
        &self,
        async_block: *mut XAsyncBlock,
        identity: *const c_void,
        buffer_size: usize,
        buffer: *mut c_void,
        buffer_used: *mut usize,
    ) -> HRESULT {
        let blk = unsafe { &mut *async_block };
        let (Some(state), Some(hr)) = blk.get_state_ex(true) else {
            return E_FAIL;
        };
        let provider_data = unsafe { &mut *state.get_provider_data() };
        let provider = unsafe { state.get_provider() };
        provider_data.buffer = buffer;
        provider_data.buffer_size = buffer_size;
        let _ = unsafe { provider(XAsyncOp::GetResult, provider_data) };

        hr
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
        handle.map(|f| unsafe { f.unregister_waiter(token) });
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
