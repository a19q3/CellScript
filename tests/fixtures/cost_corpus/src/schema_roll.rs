// Matched reference for the CellScript schema-roll relation: a two-field
// Note (32-byte owner, u64 amount) whose successor keeps the owner verbatim,
// increments the amount by exactly one, preserves the complete Lock Script
// hash, the Type Script hash and the capacity.
#![no_std]
#![no_main]

use core::alloc::{GlobalAlloc, Layout};
use ckb_std::ckb_constants::{CellField, Source};
use ckb_std::syscalls::exit;
use common::{data, field, u64_at};

mod common;

struct NoAlloc;
unsafe impl GlobalAlloc for NoAlloc {
    unsafe fn alloc(&self, _: Layout) -> *mut u8 {
        core::ptr::null_mut()
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static ALLOCATOR: NoAlloc = NoAlloc;
core::arch::global_asm!(".global _start", "_start:", "call rust_main", "li a7, 93", "ecall");

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> i8 {
    run().err().unwrap_or(0)
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    exit(99)
}

fn run() -> Result<(), i8> {
    let before = data::<40>(Source::Input)?;
    let after = data::<40>(Source::Output)?;
    if before[..32] != after[..32] {
        return Err(13);
    }
    let amount = u64_at(&before, 32)?;
    if u64_at(&after, 32)? != amount.checked_add(1).ok_or(14)? {
        return Err(15);
    }
    if field::<32>(Source::Input, CellField::LockHash)? != field::<32>(Source::Output, CellField::LockHash)? {
        return Err(16);
    }
    if field::<32>(Source::Input, CellField::TypeHash)? != field::<32>(Source::Output, CellField::TypeHash)? {
        return Err(17);
    }
    if field::<8>(Source::Input, CellField::Capacity)? != field::<8>(Source::Output, CellField::Capacity)? {
        return Err(18);
    }
    Ok(())
}
