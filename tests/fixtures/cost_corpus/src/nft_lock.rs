// Matched reference for the CellScript ownership-claim lock: the witness
// must claim exactly the input data's 32-byte owner field. This mirrors the
// repository's established Lock-parameter fixture scope: it tests Lock
// parameter sources, not an authenticated ownership policy.
#![no_std]
#![no_main]

use core::alloc::{GlobalAlloc, Layout};
use ckb_std::syscalls::exit;
use common::{data, witness_address_arg};

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
    let owner = data::<32>(ckb_std::ckb_constants::Source::Input)?;
    let claimed = witness_address_arg()?;
    if claimed != owner {
        return Err(13);
    }
    Ok(())
}
