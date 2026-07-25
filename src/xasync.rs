use crate::S_OK;
use crate::com::query_api_impl;

use crate::results::*;
use std::ffi::{c_char, c_void};
use std::mem::size_of;
use std::pin::Pin;
use std::ptr::null_mut;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use windows_core::{GUID, HRESULT, IUnknown, Interface, interface};
use windows_sys::core::BOOL;

type XTaskQueueHandle = *mut c_void;
type XAsyncCompletionRoutine = unsafe extern "system" fn(async_block: *mut XAsyncBlock);
type XAsyncProvider =
    unsafe extern "system" fn(op: XAsyncOp, data: *const XAsyncProviderData) -> HRESULT;

const CLSID_XASYNC: GUID = GUID::from_u128(0x073b7dcb_1fcf_4030_94be_e3c9eb623428);

#[repr(C)]
pub struct XAsyncBlock {
    pub queue: XTaskQueueHandle,
    pub context: *mut c_void,
    pub callback: Option<XAsyncCompletionRoutine>,
    pub internal: [u8; size_of::<*mut c_void>() * 4],
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XAsyncOp {
    Begin = 0,
    DoWork = 1,
    GetResult = 2,
    Cancel = 3,
    Cleanup = 4,
}

#[repr(C)]
pub struct XAsyncProviderData {
    pub async_: *mut XAsyncBlock,
    pub bufferSize: usize,
    pub buffer: *mut c_void,
    pub context: *mut c_void,
}

#[interface("073b7dcb-1fcf-4030-94be-e3c9eb623428")]
pub unsafe trait IXAsync: IUnknown {
    unsafe fn XAsyncGetStatus(&self, asyncBlock: *mut c_void, wait: BOOL) -> HRESULT;
    unsafe fn XAsyncGetResultSize(
        &self,
        asyncBlock: *mut c_void,
        bufferSize: *mut usize,
    ) -> HRESULT;
    unsafe fn XAsyncCancel(&self, asyncBlock: *mut c_void) -> ();
    unsafe fn XAsyncRun(&self, asyncBlock: *mut c_void, work: *mut c_void) -> HRESULT;
    unsafe fn XAsyncBegin(
        &self,
        asyncBlock: *mut c_void,
        context: *mut c_void,
        identity: *mut c_void,
        identityName: *mut c_char,
        provider: *mut c_void,
    ) -> HRESULT;
    unsafe fn __ReservedSlot8(&self) -> HRESULT;
    unsafe fn XAsyncSchedule(&self, asyncBlock: *mut c_void, delayInMs: u32) -> HRESULT;
    unsafe fn XAsyncComplete(
        &self,
        asyncBlock: *mut c_void,
        result: i32,
        requiredBufferSize: u64,
    ) -> ();
    unsafe fn XAsyncGetResult(
        &self,
        asyncBlock: *mut c_void,
        identity: *mut c_void,
        bufferSize: u64,
        buffer: *mut c_void,
        bufferUsed: *mut usize,
    ) -> HRESULT;
    unsafe fn XTaskQueueCreate(
        &self,
        workDispatchMode: u64,
        completionDispatchMode: u64,
        queue: *mut u64,
    ) -> HRESULT;
    unsafe fn XTaskQueueCreateComposite(
        &self,
        workPort: u64,
        completionPort: u64,
        queue: *mut u64,
    ) -> HRESULT;
    unsafe fn XTaskQueueGetPort(&self, queue: u64, port: u64, portHandle: *mut u64) -> HRESULT;
    unsafe fn XTaskQueueDuplicateHandle(
        &self,
        queueHandle: u64,
        duplicatedHandle: *mut u64,
    ) -> HRESULT;
    unsafe fn XTaskQueueDispatch(&self, queue: u64, port: u64, timeoutInMs: u32) -> BOOL;
    unsafe fn XTaskQueueCloseHandle(&self, queue: u64) -> ();
    unsafe fn XTaskQueueSubmitCallback(
        &self,
        queue: u64,
        port: u64,
        callbackContext: *mut c_void,
        callback: *mut c_void,
    ) -> HRESULT;
    unsafe fn XTaskQueueSubmitDelayedCallback(
        &self,
        queue: u64,
        port: u64,
        delayMs: u32,
        callbackContext: *mut c_void,
        callback: *mut c_void,
    ) -> HRESULT;
    unsafe fn XTaskQueueRegisterWaiter(
        &self,
        queue: u64,
        port: u64,
        waitHandle: *mut c_void,
        callbackContext: *mut c_void,
        callback: *mut c_void,
        token: *mut u64,
    ) -> HRESULT;
    unsafe fn XTaskQueueUnregisterWaiter(&self, queue: u64, token: u64) -> ();
    unsafe fn XTaskQueueTerminate(
        &self,
        queue: u64,
        wait: BOOL,
        callbackContext: *mut c_void,
        callback: *mut c_void,
    ) -> HRESULT;
    unsafe fn XTaskQueueRegisterMonitor(
        &self,
        queue: u64,
        callbackContext: *mut c_void,
        callback: *mut c_void,
        token: *mut u64,
    ) -> HRESULT;
    unsafe fn XTaskQueueUnregisterMonitor(&self, queue: u64, token: u64) -> ();
    unsafe fn XTaskQueueGetCurrentProcessTaskQueue(&self, queue: *mut u64) -> BOOL;
    unsafe fn XTaskQueueSetCurrentProcessTaskQueue(&self, queue: u64) -> ();
    unsafe fn XThreadSetTimeSensitive(&self, isTimeSensitiveThread: BOOL) -> HRESULT;
    unsafe fn __ReservedSlot28(&self) -> HRESULT;
    unsafe fn XThreadAssertNotTimeSensitive(&self) -> ();
    unsafe fn XThreadIsTimeSensitive(&self) -> BOOL;
}

fn interface() -> Result<IXAsync, HRESULT> {
    let mut out = std::ptr::null_mut();
    let hr = query_api_impl(&CLSID_XASYNC, &IXAsync::IID, &mut out);
    if hr != S_OK {
        return Err(hr);
    }
    Ok(unsafe { IXAsync::from_raw(out) })
}

unsafe fn begin(
    async_block: *mut XAsyncBlock,
    context: *mut c_void,
    identity: *const c_void,
    identity_name: *const c_char,
    provider: XAsyncProvider,
) -> HRESULT {
    let xasync = match interface() {
        Ok(xasync) => xasync,
        Err(hr) => return hr,
    };
    let hr = unsafe {
        xasync.XAsyncBegin(
            async_block.cast(),
            context,
            identity.cast_mut(),
            identity_name.cast_mut(),
            provider as *mut c_void,
        )
    };
    std::mem::forget(xasync);
    hr
}

unsafe fn schedule(async_block: *mut XAsyncBlock, delay_ms: u32) -> HRESULT {
    let xasync = match interface() {
        Ok(xasync) => xasync,
        Err(hr) => return hr,
    };
    let hr = unsafe { xasync.XAsyncSchedule(async_block.cast(), delay_ms) };
    std::mem::forget(xasync);
    hr
}

unsafe fn complete(async_block: *mut XAsyncBlock, result: HRESULT, required_buffer_size: usize) {
    let xasync = match interface() {
        Ok(xasync) => xasync,
        Err(_) => return,
    };
    unsafe { xasync.XAsyncComplete(async_block.cast(), result.0, required_buffer_size as u64) };
    std::mem::forget(xasync);
}

pub(crate) unsafe fn get_result<T>(
    async_block: *mut XAsyncBlock,
    identity: *const c_void,
    out: *mut T,
) -> HRESULT {
    let xasync = match interface() {
        Ok(xasync) => xasync,
        Err(hr) => return hr,
    };
    let mut buffer_used = 0usize;
    let hr = unsafe {
        xasync.XAsyncGetResult(
            async_block.cast(),
            identity.cast_mut(),
            size_of::<T>() as u64,
            out.cast(),
            &mut buffer_used,
        )
    };
    std::mem::forget(xasync);
    hr
}

pub(crate) unsafe fn get_status(async_block: *mut XAsyncBlock, wait: bool) -> HRESULT {
    let xasync = match interface() {
        Ok(xasync) => xasync,
        Err(hr) => return hr,
    };
    let hr = unsafe { xasync.XAsyncGetStatus(async_block.cast(), wait.into()) };
    std::mem::forget(xasync);
    hr
}

pub(crate) unsafe fn get_result_size(async_block: *mut XAsyncBlock) -> Result<usize, HRESULT> {
    let xasync = match interface() {
        Ok(xasync) => xasync,
        Err(hr) => return Err(hr),
    };
    let mut buffer_size: usize = 0;
    let hr = unsafe { xasync.XAsyncGetResultSize(async_block.cast(), &mut buffer_size) };
    std::mem::forget(xasync);
    if hr == S_OK { Ok(buffer_size) } else { Err(hr) }
}

struct XAsyncContextHelper<T: Sized> {
    result: HRESULT,
    canceled: bool,
    payload: Option<T>,
    future: Pin<Box<dyn Future<Output = Result<T, HRESULT>> + Send + 'static>>,
}

struct XAsyncWaker {
    block: *mut XAsyncBlock,
}

unsafe impl Sync for XAsyncWaker {}
unsafe impl Send for XAsyncWaker {}

impl Wake for XAsyncWaker {
    fn wake(self: Arc<Self>) {
        // println!("wake");
        unsafe { schedule(self.block, 0) };
    }
}

unsafe extern "system" fn run_async_helper<T: Sized>(
    op: XAsyncOp,
    data: *const XAsyncProviderData,
) -> HRESULT {
    let Some(data) = (unsafe { data.as_ref() }) else {
        return E_POINTER;
    };
    let async_context = data.context as *mut XAsyncContextHelper<T>;
    let Some(async_context) = (unsafe { async_context.as_mut() }) else {
        return E_POINTER;
    };

    match op {
        XAsyncOp::Begin => unsafe { schedule(data.async_, 0) },
        XAsyncOp::DoWork => {
            if async_context.canceled {
                async_context.result = E_ABORT;
            } else {
                let waker = Waker::from(Arc::new(XAsyncWaker { block: data.async_ }));
                let mut cx = Context::from_waker(&waker);
                match async_context.future.as_mut().poll(&mut cx) {
                    Poll::Ready(value) => {
                        match value {
                            Err(hr) => async_context.result = hr,
                            Ok(value) => {
                                async_context.result = S_OK;
                                async_context.payload = Some(value);
                            }
                        };
                    }
                    Poll::Pending => {
                        // println!("pending");
                        return E_PENDING;
                    }
                }
            }
            // println!("required_buf_size {}", size_of::<T>());
            unsafe {
                complete(data.async_, async_context.result, size_of::<T>());
            }
            S_OK
        }
        XAsyncOp::GetResult => {
            // println!("get_result {}", size_of::<T>());
            if async_context.result == S_OK
                && let Some(payload) = &async_context.payload
            {
                // println!("copy result {}", size_of::<T>());
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        (payload as *const T).cast::<u8>(),
                        data.buffer.cast::<u8>(),
                        size_of::<T>(),
                    );
                }
            }
            S_OK
        }
        XAsyncOp::Cancel => {
            async_context.canceled = true;
            S_OK
        }
        XAsyncOp::Cleanup => {
            unsafe {
                drop(Box::from_raw(async_context));
            }
            S_OK
        }
    }
}

pub unsafe fn run<T: Sized, F>(async_: *mut XAsyncBlock, future: F) -> HRESULT
where
    F: Future<Output = Result<T, HRESULT>> + Send + 'static,
{
    if async_.is_null() {
        return S_OK;
    }

    let async_context = Box::new(XAsyncContextHelper {
        canceled: false,
        payload: None as Option<T>,
        result: E_ABORT,
        future: Box::pin(future),
    });
    let async_context = Box::into_raw(async_context);
    let hr = unsafe {
        begin(
            async_,
            async_context.cast(),
            null_mut(),
            c"run_async".as_ptr(),
            run_async_helper::<T>,
        )
    };
    if hr != S_OK {
        unsafe {
            drop(Box::from_raw(async_context));
        }
    }
    hr
}

struct XsyncContextHelper<T: Sized, F: Fn() -> Result<T, HRESULT>> {
    result: HRESULT,
    canceled: bool,
    payload: Option<T>,
    future: F,
}

unsafe extern "system" fn run_sync_helper<T: Sized, F: Fn() -> Result<T, HRESULT>>(
    op: XAsyncOp,
    data: *const XAsyncProviderData,
) -> HRESULT {
    let Some(data) = (unsafe { data.as_ref() }) else {
        return E_POINTER;
    };
    let async_context = data.context as *mut XsyncContextHelper<T, F>;
    let Some(async_context) = (unsafe { async_context.as_mut() }) else {
        return E_POINTER;
    };

    match op {
        XAsyncOp::Begin => unsafe {
            let value = (async_context.future)();
            match value {
                Err(hr) => async_context.result = hr,
                Ok(value) => {
                    async_context.result = S_OK;
                    async_context.payload = Some(value);
                }
            };
            complete(data.async_, async_context.result, size_of::<T>());
            S_OK
        },
        XAsyncOp::DoWork => S_OK,
        XAsyncOp::GetResult => {
            // println!("get_result {}", size_of::<T>());
            if async_context.result == S_OK
                && let Some(payload) = &async_context.payload
            {
                // println!("copy result {}", size_of::<T>());
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        (payload as *const T).cast::<u8>(),
                        data.buffer.cast::<u8>(),
                        size_of::<T>(),
                    );
                }
            }
            S_OK
        }
        XAsyncOp::Cancel => {
            async_context.canceled = true;
            S_OK
        }
        XAsyncOp::Cleanup => {
            unsafe {
                drop(Box::from_raw(async_context));
            }
            S_OK
        }
    }
}

pub unsafe fn run_sync<T: Sized, F>(async_: *mut XAsyncBlock, future: F) -> HRESULT
where
    F: Fn() -> Result<T, HRESULT>,
{
    if async_.is_null() {
        return S_OK;
    }

    let async_context = Box::new(XsyncContextHelper {
        canceled: false,
        payload: None as Option<T>,
        result: E_ABORT,
        future: future,
    });
    let async_context = Box::into_raw(async_context);
    let hr = unsafe {
        begin(
            async_,
            async_context.cast(),
            null_mut(),
            c"run_async".as_ptr(),
            run_sync_helper::<T, F>,
        )
    };
    if hr != S_OK {
        unsafe {
            drop(Box::from_raw(async_context));
        }
    }
    hr
}
