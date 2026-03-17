pub mod interuption_impl {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    /**
     * This trait was created for defining how execution of wain-exec can be
     * interupted to take a snapshot of WebAssembly-modules runtime.
     * wain-exec can ask the implementer of this trait if an interuption has been requested
     */
    pub trait Interuption {
        fn check_interuption(&self) -> bool;
    }

    /**
     * The purpose of this struct is to store information related to
     * interuption method and to allow implementation of check_interuption function.
     *
     * Production version could read socket for interption comming from orchestrator etc.
     */
    pub struct Implementer {
        interupted: Arc<AtomicBool>,
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl Interuption for Implementer {
        fn check_interuption(&self) -> bool {
            self.interupted.load(Ordering::Relaxed)
        }
    }

    impl Implementer {
        pub fn new(flag: Arc<AtomicBool>, snapshot_bytes: Arc<Mutex<Vec<u8>>>) -> Self {
            Self {
                interupted: flag,
                buf: snapshot_bytes,
            }
        }

        pub fn interupt(&self) {
            self.interupted.store(true, Ordering::Relaxed);
        }

        pub fn store_snapshot_bytes(&self, bytes: Vec<u8>) {
            let mut guarded = self.buf.lock().unwrap();
            *guarded = bytes;
        }
    }
}
