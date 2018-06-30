use std::{mem, thread};

use crossbeam_channel::{bounded, Receiver, Sender};

pub struct Jobs {
    jobs: Vec<ConcurrentJob>,
}

impl Jobs {
    pub fn new() -> Jobs {
        Jobs { jobs: Vec::new() }
    }

    pub fn add(&mut self, job: ConcurrentJob) {
        self.gc();
        self.jobs.push(job);
    }

    pub fn wait_for_all(&mut self) {
        while !self.jobs.is_empty() {
            let done = {
                let chans = self.jobs.iter().map(|j| &j.chan);
                select! {
                    recv(chans, msg, from) => {
                        assert!(msg.is_none());
                        self.jobs.iter().position(|j| &j.chan == from).unwrap()
                    }
                }
            };
            drop(self.jobs.swap_remove(done));
        }
    }

    fn gc(&mut self) {
        self.jobs.retain(|job| !job.is_completed())
    }
}

impl Drop for Jobs {
    fn drop(&mut self) {
        let jobs = mem::replace(&mut self.jobs, Vec::new());
        for job in jobs {
            job.abandon()
        }
    }
}

#[derive(Clone)]
#[must_use]
pub struct ConcurrentJob {
    is_abandoned: bool,
    chan: Receiver<Never>,
}

pub struct JobToken {
    #[allow(unused)] // for drop
    chan: Sender<Never>,
}

impl ConcurrentJob {
    pub fn new() -> (ConcurrentJob, JobToken) {
        let (tx, rx) = bounded(0);
        let job = ConcurrentJob {
            chan: rx,
            is_abandoned: false,
        };
        let token = JobToken { chan: tx };
        (job, token)
    }

    fn is_completed(&self) -> bool {
        select! {
            recv(self.chan, msg) => match msg {
                None => true,
                Some(never) => match never {}
            }
            default => false,
        }
    }

    fn abandon(mut self) {
        self.is_abandoned = true
    }
}

impl Drop for ConcurrentJob {
    fn drop(&mut self) {
        if self.is_abandoned || self.is_completed() || thread::panicking(){
            return;
        }
        panic!("orphaned concurrent job");
    }
}

enum Never {}
