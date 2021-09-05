use std::alloc::GlobalAlloc;

pub trait InternalAlloc
where
    Self: GlobalAlloc,
{
    fn allocate<T>(&self, layout: std::alloc::Layout) -> *mut T {
        let ptr = unsafe { GlobalAlloc::alloc(self, layout) } as *mut T;
        ptr
    }

    fn free<T>(&self, ptr: *mut T, layout: std::alloc::Layout) {
        unsafe { GlobalAlloc::dealloc(self, ptr as *mut u8, layout) };
    }
}

impl InternalAlloc for std::alloc::System {}
