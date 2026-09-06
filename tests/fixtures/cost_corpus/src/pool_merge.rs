// Matched reference for the CellScript two-input pool merge: both input
// amounts positive, checked sum, output amount equal to the sum, and the
// output's complete Lock Script hash bound to the witness recipient. Same
// scope as the CellScript fixture: no capacity or issuance policy.
#![no_std]
#![no_main]

use core::alloc::{GlobalAlloc, Layout};
use ckb_std::ckb_constants::{CellField, Source};
use ckb_std::syscalls::exit;
use common::{data_at, field, u64_at, witness_address_arg};

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
    let left = u64_at(&data_at(Source::Input, 0)?, 0)?;
    let right = u64_at(&data_at(Source::Input, 1)?, 0)?;
    let merged = u64_at(&data_at(Source::Output, 0)?, 0)?;
    if left == 0 || right == 0 {
        return Err(13);
    }
    let total = left.checked_add(right).ok_or(14)?;
    if merged != total {
        return Err(15);
    }
    let recipient = witness_address_arg()?;
    if field::<32>(Source::Output, CellField::LockHash)? != recipient {
        return Err(16);
    }
    Ok(())
}
