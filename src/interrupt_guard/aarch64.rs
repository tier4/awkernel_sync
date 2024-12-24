use core::arch::asm;

#[inline(always)]
pub fn get_flag() -> usize {
    let v: u64;
    unsafe { asm!("mrs {}, daif", lateout(reg) v) };
    v as usize
}

#[inline(always)]
pub fn disable() {
    unsafe { core::arch::asm!("msr daifset, #0b0010",) };
}

#[inline(always)]
pub fn set_flag(flag: usize) {
    asm!("msr daif, {}", in(reg) v as u64);
}
