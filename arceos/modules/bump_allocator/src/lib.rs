#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
///
pub struct EarlyAllocator<const SIZE: usize> {
    start: usize,
    end: usize,
    b_pos: usize,
    p_pos: usize,
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }
}

const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

const fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        self.start = start;
        self.end = start + size;
        self.b_pos = start;
        self.p_pos = self.end;
        self.count = 0;
    }

    fn add_memory(&mut self, _start: usize, _size: usize) -> AllocResult {
        // The early allocator only manages a single continuous region.
        Err(AllocError::NoMemory)
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        // Bytes are allocated from the low end, growing upwards.
        let pos = align_up(self.b_pos, layout.align().max(1));
        let new_pos = pos.checked_add(layout.size()).ok_or(AllocError::NoMemory)?;
        if new_pos > self.p_pos {
            return Err(AllocError::NoMemory);
        }

        self.b_pos = new_pos;
        self.count += 1;
        NonNull::new(pos as *mut u8).ok_or(AllocError::NoMemory)
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        // The bump allocator cannot free a single allocation. It only resets
        // the whole bytes area once every allocation has been released.
        self.count = self.count.saturating_sub(1);
        if self.count == 0 {
            self.b_pos = self.start;
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.p_pos - self.b_pos
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        // `align_pow2` is the alignment in bytes, which must be a power of two.
        if num_pages == 0 || !align_pow2.is_power_of_two() {
            return Err(AllocError::InvalidParam);
        }

        // Pages are allocated from the high end, growing downwards.
        let size = num_pages.checked_mul(SIZE).ok_or(AllocError::NoMemory)?;
        let pos = self.p_pos.checked_sub(size).ok_or(AllocError::NoMemory)?;
        let pos = align_down(pos, align_pow2.max(SIZE));
        if pos < self.b_pos {
            return Err(AllocError::NoMemory);
        }

        self.p_pos = pos;
        Ok(pos)
    }

    fn dealloc_pages(&mut self, _pos: usize, _num_pages: usize) {
        // Pages are never freed in the early allocator.
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / SIZE
    }
}
