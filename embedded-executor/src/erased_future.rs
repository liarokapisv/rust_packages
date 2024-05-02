use core::{future::Future, pin::Pin, ptr::NonNull};

#[derive(Debug)]
pub struct ErasedFuture {
    #[doc(hidden)]
    pub __pointer: NonNull<dyn Future<Output = ()> + Send>,
}

impl ErasedFuture {
    // # Safety
    // The underlying object must be alive
    pub unsafe fn get_pinned(&mut self) -> Pin<&mut (dyn Future<Output = ()> + Send)> {
        Pin::new_unchecked(self.__pointer.as_mut())
    }
}

impl Drop for ErasedFuture {
    fn drop(&mut self) {
        println!("erased future dropped")
    }
}

// # Safety
// The ErasedFuture is not aliased so is safe to Send.
unsafe impl Send for ErasedFuture {}

// # Safety
// The ErasedFuture is only accessed through a mutable reference and so is safe to Sync. Kind of
// pointless to be fair.
unsafe impl Sync for ErasedFuture {}

#[macro_export]
macro_rules! erase_future {
    ($id:ident, $future:expr) => {
        let mut future = $future;
        let $id = crate::erased_future::ErasedFuture {
            __pointer: core::ptr::NonNull::from(unsafe {
                core::mem::transmute::<
                    &mut (dyn Future<Output = ()> + Send + '_),
                    &mut (dyn Future<Output = ()> + Send + 'static),
                >(&mut future)
            }),
        };
    };
}
