//#![no_std]
#![allow(dead_code)]
#![feature(never_type)]
#![feature(waker_getters)]

mod erased_future;
mod yield_now;

use core::{
    future::Future,
    mem::transmute,
    pin::Pin,
    task::{RawWaker, RawWakerVTable, Waker},
};
use erased_future::ErasedFuture;
use spin::mutex::SpinMutex;
use std::{
    println,
    sync::atomic::{AtomicUsize, Ordering},
};

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListAtomicLink, UnsafeRef};

use core::task::Context;

use yield_now::yield_now;

#[derive(Debug, Clone)]
struct AliveExecutorWaker
where
    Self: Send + Sync,
{
    task: Pin<UnsafeRef<PinnedTask>>,
    queue: UnsafeRef<SpinMutex<LinkedList<PinnedTaskAdapter>>>,
}

#[derive(Debug, Clone)]
enum ExecutorWaker
where
    Self: Send + Sync,
{
    Dead,
    Alive(AliveExecutorWaker),
}

impl ExecutorWaker {
    fn wake(self) {
        if let ExecutorWaker::Alive(waker) = self {
            let mut queue = waker.queue.lock();
            queue.push_back(waker.task);
        }
    }

    fn wake_by_ref(&self) {
        if let ExecutorWaker::Alive(waker) = self {
            let mut queue = waker.queue.lock();
            queue.push_back(waker.task.clone());
        }
    }
}

#[derive(Debug)]
struct PinnedTask
where
    Self: Sync + Sync,
{
    future: SpinMutex<ErasedFuture>,
    waker: SpinMutex<Option<ExecutorWaker>>,
    link: LinkedListAtomicLink,
}

impl Drop for PinnedTask {
    fn drop(&mut self) {
        println!("task dropped");
    }
}

#[derive(Debug, Default)]
struct Executor
where
    Self: Send + Sync,
{
    queue: SpinMutex<LinkedList<PinnedTaskAdapter>>,
}

intrusive_adapter!(PinnedTaskAdapter = Pin<UnsafeRef<PinnedTask>>: PinnedTask { link: LinkedListAtomicLink });

// SAFETY:
//
// These should safe for the most part

unsafe fn clone_waker(w: *const ()) -> RawWaker {
    RawWaker::new(w, &VTABLE)
}

unsafe fn drop_waker(_: *const ()) {}

unsafe fn wake_waker(w: *const ()) {
    wake_waker_by_ref(w);
}

unsafe fn wake_waker_by_ref(w: *const ()) {
    let waker = { &*(w as *const ExecutorWaker) };
    waker.wake_by_ref();
}

const VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_waker, wake_waker, wake_waker_by_ref, drop_waker);

// Invariants:
//
// When a pinned task goes out of scope, an unamed drop impl unsubscribes it from the queue.
// The tasks are only pinnable through the provided macro and they cannot be leaked.
//
// TODO!
// See the Drop impl for drop safety issues.

impl Executor {
    pub fn new() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn __push_back(&self, task: Pin<UnsafeRef<PinnedTask>>) -> impl Drop {
        let mut queue = self.queue.lock();
        queue.push_back(task);

        struct RemoveHandle {
            queue: UnsafeRef<SpinMutex<LinkedList<PinnedTaskAdapter>>>,
            cursor_as_ptr: *const PinnedTask,
        }

        impl Drop for RemoveHandle {
            fn drop(&mut self) {
                let mut queue = self.queue.lock();
                let mut cursor = unsafe {
                    // SAFETY
                    // The pointer is valid.
                    queue.cursor_mut_from_ptr(self.cursor_as_ptr)
                };
                cursor.remove();
            }
        }

        RemoveHandle {
            queue: unsafe {
                // Since the executor must outlive any tasks, the queue should still be alive when
                // the RemoveHandle is Dropped.
                UnsafeRef::from_raw(&self.queue)
            },
            cursor_as_ptr: queue.back().get().unwrap(),
        }
    }

    pub fn poll(&self) {
        let task = self.queue.lock().pop_front();
        if let Some(task) = task {
            let task = unsafe {
                // # Safety
                // Task is pinned and we do not move it.
                Pin::into_inner_unchecked(task)
            };
            let task = task.as_ref();
            let waker = {
                let raw_waker = {
                    let mut waker = task.waker.lock();
                    let exec_waker = waker.insert(ExecutorWaker::Alive(AliveExecutorWaker {
                        task: unsafe {
                            // # Safety
                            // Tasks are pinned so we can pin them safely.
                            // We only ever access task through a shared reference.
                            // Task is still alive due to the auto-removal guarantee.
                            Pin::new_unchecked(UnsafeRef::from_raw(task))
                        },
                        queue: unsafe {
                            // # Safety
                            // Queue is always accessed through a shared reference (Mutex)
                            // Any references that persist within the wakers inside the tasks
                            // will be purged when the executor is dropped.
                            UnsafeRef::from_raw(&self.queue)
                        },
                    }));
                    let raw = RawWaker::new(
                        // # Safety
                        // Casting to properly pass the data that will be used with the vtable.
                        unsafe {
                            transmute::<*const ExecutorWaker, *const ()>(
                                exec_waker as *const ExecutorWaker,
                            )
                        },
                        &VTABLE,
                    );
                    drop(waker);
                    raw
                };
                unsafe {
                    // # Safety
                    // Underlying waker is Send + Sync and is constructed from a valid vtable.
                    Waker::from_raw(raw_waker)
                }
            };
            let mut context = Context::from_waker(&waker);
            let mut future = task.future.lock();
            let pinned_future = unsafe {
                // # Safety
                // The underlying object is alive otherwise, it would auto-remove itself
                future.get_pinned()
            };
            let _ = pinned_future.poll(&mut context);
        }
    }
}

// NOTE:
// This is needed in case the executor is manually dropped before
// the tasks are dropped. If a task is woken up through its Waker,
// the waker would access invalid memory.
// This is a workaround for the common case but it is not guaranteed to be
// called. Maybe access through a macro?

impl Drop for Executor {
    fn drop(&mut self) {
        let queue = self.queue.lock();
        for task in queue.iter() {
            let mut waker = task.waker.lock();
            *waker = Some(ExecutorWaker::Dead);
        }
    }
}

macro_rules! spawn_task {
    ($executor:expr, $future:expr) => {
        erase_future!(future, $future);
        yield_now().await;
        let task = PinnedTask {
            future: SpinMutex::new(future),
            waker: SpinMutex::new(None),
            link: LinkedListAtomicLink::new(),
        };
        let pinned_task = unsafe { Pin::new_unchecked(UnsafeRef::from_raw(&task)) };
        let _handle = $executor.__push_back(pinned_task);
    };
}

#[tokio::main]
async fn main() {
    let i: AtomicUsize = 0.into();
    let executor = Executor::new();
    spawn_task!(executor, async {
        let mut buffer = [0; 1000];
        loop {
            let j = i.load(Ordering::Acquire);
            buffer[j % 1000] = j;
            println!("{}", buffer[j]);
            yield_now().await
        }
    });
    spawn_task!(executor, async {
        loop {
            let mut j = i.load(Ordering::Relaxed);
            j += 1;
            if j == 1000 {
                j = 0;
            }
            i.store(j, Ordering::Release);
            yield_now().await
        }
    });
    std::thread::scope(|scope| {
        scope.spawn(|| loop {
            executor.poll();
        });
    });
}
