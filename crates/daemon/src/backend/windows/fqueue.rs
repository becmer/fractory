// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2025 Kamil Becmer

use std::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use ringbuf::{HeapRb, producer::Producer, traits::Consumer};

pub trait Factory {
    type Seed;
    type Item;
    fn create(&mut self) -> Self::Seed;
    fn settle(&mut self, seed: Self::Seed) -> Self::Item;
}
impl<T, F> Factory for F
where
    F: FnMut() -> T,
{
    type Seed = T;
    type Item = T;
    fn create(&mut self) -> T {
        self()
    }
    fn settle(&mut self, seed: T) -> T {
        seed
    }
}

pub struct FactoryQueue<F: Factory> {
    f: F,
    buf: HeapRb<F::Seed>,
}

impl<F: Factory> FactoryQueue<F> {
    pub fn from<I: Into<F>>(f: I, capacity: usize) -> Self {
        let mut f = f.into();
        let mut buf = Box::<[F::Seed]>::new_uninit_slice(capacity);
        let ptr = buf.as_mut_ptr();
        for i in 0..capacity {
            let seed = f.create();
            unsafe { ptr.add(i).write(MaybeUninit::new(seed)) };
        }
        let buf = HeapRb::<F::Seed>::from(unsafe { buf.assume_init() });
        Self { f, buf }
    }

    pub fn next(&mut self) -> F::Item {
        let last_seed = self.f.create();
        let next_seed = match self.buf.try_pop() {
            Some(next_seed) => {
                let _ = self.buf.try_push(last_seed);
                next_seed
            }
            None => last_seed,
        };
        self.f.settle(next_seed)
    }
}

impl<F: Factory> Deref for FactoryQueue<F>
where
    F: Deref,
{
    type Target = F::Target;

    fn deref(&self) -> &Self::Target {
        &self.f
    }
}

impl<F: Factory> DerefMut for FactoryQueue<F>
where
    F: DerefMut,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.f
    }
}
