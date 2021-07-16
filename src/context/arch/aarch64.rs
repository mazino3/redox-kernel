use core::mem;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::device::cpu::registers::{control_regs, tlb};
use crate::syscall::FloatRegisters;

/// This must be used by the kernel to ensure that context switches are done atomically
/// Compare and exchange this to true when beginning a context switch on any CPU
/// The `Context::switch_to` function will set it back to false, allowing other CPU's to switch
/// This must be done, as no locks can be held on the stack during switch
pub static CONTEXT_SWITCH_LOCK: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub struct Context {
    elr_el1: usize,
    sp_el0: usize,
    ttbr0_el1: usize,   /* Pointer to U4 translation table for this Context     */
    ttbr1_el1: usize,   /* Pointer to P4 translation table for this Context     */
    tpidr_el0: usize,   /* Pointer to TLS region for this Context               */
    tpidrro_el0: usize, /* Pointer to TLS (read-only) region for this Context   */
    spsr_el1: usize,
    esr_el1: usize,
    fx_loadable: bool,
    fx_address: usize,
    sp: usize,          /* Stack Pointer (x31)                                  */
    lr: usize,          /* Link Register (x30)                                  */
    fp: usize,          /* Frame pointer Register (x29)                         */
    x28: usize,         /* Callee saved Register                                */
    x27: usize,         /* Callee saved Register                                */
    x26: usize,         /* Callee saved Register                                */
    x25: usize,         /* Callee saved Register                                */
    x24: usize,         /* Callee saved Register                                */
    x23: usize,         /* Callee saved Register                                */
    x22: usize,         /* Callee saved Register                                */
    x21: usize,         /* Callee saved Register                                */
    x20: usize,         /* Callee saved Register                                */
    x19: usize,         /* Callee saved Register                                */
    x18: usize,
    x17: usize,
    x16: usize,
    x15: usize,         /* Temporary Register                                   */
    x14: usize,         /* Temporary Register                                   */
    x13: usize,         /* Temporary Register                                   */
    x12: usize,         /* Temporary Register                                   */
    x11: usize,         /* Temporary Register                                   */
    x10: usize,         /* Temporary Register                                   */
    x9: usize,          /* Temporary Register                                   */
    x8: usize,          /* Indirect location Register                           */
}

impl Context {
    pub fn new() -> Context {
        Context {
            elr_el1: 0,
            sp_el0: 0,
            ttbr0_el1: 0,
            ttbr1_el1: 0,
            tpidr_el0: 0,
            tpidrro_el0: 0,
            spsr_el1: 0,
            esr_el1: 0,
            fx_loadable: false,
            fx_address: 0,
            sp: 0,
            lr: 0,
            fp: 0,
            x28: 0,
            x27: 0,
            x26: 0,
            x25: 0,
            x24: 0,
            x23: 0,
            x22: 0,
            x21: 0,
            x20: 0,
            x19: 0,
            x18: 0,
            x17: 0,
            x16: 0,
            x15: 0,
            x14: 0,
            x13: 0,
            x12: 0,
            x11: 0,
            x10: 0,
            x9: 0,
            x8: 0,
        }
    }

    pub fn get_page_utable(&self) -> usize {
        self.ttbr0_el1
    }

    pub fn get_page_ktable(&self) -> usize {
        self.ttbr1_el1
    }

    pub fn set_page_utable(&mut self, address: usize) {
        self.ttbr0_el1 = address;
    }

    pub fn set_page_ktable(&mut self, address: usize) {
        self.ttbr1_el1 = address;
    }

    pub fn set_stack(&mut self, address: usize) {
        self.sp = address;
    }

    pub fn set_lr(&mut self, address: usize) {
        self.lr = address;
    }

    pub fn set_tcb(&mut self, pid: usize) {
        self.tpidr_el0 = (crate::USER_TCB_OFFSET + pid * crate::PAGE_SIZE);
    }

    pub fn set_fp(&mut self, address: usize) {
        self.fp = address;
    }

    pub fn set_context_handle(&mut self) {
        let address = self as *const _ as usize;
        self.tpidrro_el0 = address;
    }

    pub fn get_context_handle(&mut self) -> usize {
        self.tpidrro_el0
    }

    pub unsafe fn signal_stack(&mut self, handler: extern fn(usize), sig: u8) {
        self.push_stack(sig as usize);
        self.push_stack(handler as usize);
        let lr = self.lr.clone();
        self.push_stack(lr);
        self.set_lr(signal_handler_wrapper as usize);
    }

    pub unsafe fn push_stack(&mut self, value: usize) {
        self.sp -= 1 * mem::size_of::<usize>();
        *(self.sp as *mut usize) = value;
    }

    pub unsafe fn pop_stack(&mut self) -> usize {
        let value = *(self.sp as *const usize);
        self.sp += 1 * mem::size_of::<usize>();
        value
    }

    pub fn get_fx_regs(&self) -> Option<FloatRegisters> {
        if !self.fx_loadable {
            return None;
        }
        let mut regs = unsafe { *(self.fx_address as *const FloatRegisters) };
        let mut new_st = regs.fp_simd_regs;
        regs.fp_simd_regs = new_st;
        Some(regs)
    }

    pub fn set_fx_regs(&mut self, mut new: FloatRegisters) -> bool {
        if !self.fx_loadable {
            return false;
        }

        {
            let old = unsafe { &*(self.fx_address as *const FloatRegisters) };
            let old_st = new.fp_simd_regs;
            let mut new_st = new.fp_simd_regs;
            for (new_st, old_st) in new_st.iter_mut().zip(&old_st) {
                *new_st = *old_st;
            }
            new.fp_simd_regs = new_st;

            // Make sure we don't use `old` from now on
        }

        unsafe {
            *(self.fx_address as *mut FloatRegisters) = new;
        }
        true
    }

    pub fn set_fx(&mut self, address: usize) {
        self.fx_address = address;
    }


    pub fn dump(&self) {
        println!("elr_el1: 0x{:016x}", self.elr_el1);
        println!("sp_el0: 0x{:016x}", self.sp_el0);
        println!("ttbr0_el1: 0x{:016x}", self.ttbr0_el1);
        println!("ttbr1_el1: 0x{:016x}", self.ttbr1_el1);
        println!("tpidr_el0: 0x{:016x}", self.tpidr_el0);
        println!("tpidrro_el0: 0x{:016x}", self.tpidrro_el0);
        println!("spsr_el1: 0x{:016x}", self.spsr_el1);
        println!("esr_el1: 0x{:016x}", self.esr_el1);
        println!("sp: 0x{:016x}", self.sp);
        println!("lr: 0x{:016x}", self.lr);
        println!("fp: 0x{:016x}", self.fp);
        println!("x28: 0x{:016x}", self.x28);
        println!("x27: 0x{:016x}", self.x27);
        println!("x26: 0x{:016x}", self.x26);
        println!("x25: 0x{:016x}", self.x25);
        println!("x24: 0x{:016x}", self.x24);
        println!("x23: 0x{:016x}", self.x23);
        println!("x22: 0x{:016x}", self.x22);
        println!("x21: 0x{:016x}", self.x21);
        println!("x20: 0x{:016x}", self.x20);
        println!("x19: 0x{:016x}", self.x19);
        println!("x18: 0x{:016x}", self.x18);
        println!("x17: 0x{:016x}", self.x17);
        println!("x16: 0x{:016x}", self.x16);
        println!("x15: 0x{:016x}", self.x15);
        println!("x14: 0x{:016x}", self.x14);
        println!("x13: 0x{:016x}", self.x13);
        println!("x12: 0x{:016x}", self.x12);
        println!("x11: 0x{:016x}", self.x11);
        println!("x10: 0x{:016x}", self.x10);
        println!("x9: 0x{:016x}", self.x9);
        println!("x8: 0x{:016x}", self.x8);
    }
}

#[cold]
#[inline(never)]
#[naked]
pub unsafe extern "C" fn switch_to(prev: &mut Context, next: &mut Context) {
}

#[allow(dead_code)]
#[repr(packed)]
pub struct SignalHandlerStack {
    x28: usize,         /* Callee saved Register                                */
    x27: usize,         /* Callee saved Register                                */
    x26: usize,         /* Callee saved Register                                */
    x25: usize,         /* Callee saved Register                                */
    x24: usize,         /* Callee saved Register                                */
    x23: usize,         /* Callee saved Register                                */
    x22: usize,         /* Callee saved Register                                */
    x21: usize,         /* Callee saved Register                                */
    x20: usize,         /* Callee saved Register                                */
    x19: usize,         /* Callee saved Register                                */
    x18: usize,
    x17: usize,
    x16: usize,
    x15: usize,         /* Temporary Register                                   */
    x14: usize,         /* Temporary Register                                   */
    x13: usize,         /* Temporary Register                                   */
    x12: usize,         /* Temporary Register                                   */
    x11: usize,         /* Temporary Register                                   */
    x10: usize,         /* Temporary Register                                   */
    x9: usize,          /* Temporary Register                                   */
    x8: usize,          /* Indirect location Register                           */
    x7: usize,
    x6: usize,
    x5: usize,
    x4: usize,
    x3: usize,
    x2: usize,
    x1: usize,
    x0: usize,
    lr: usize,
    handler: extern fn(usize),
    sig: usize,
}

#[naked]
unsafe extern fn signal_handler_wrapper() {
    #[inline(never)]
    unsafe fn inner(stack: &SignalHandlerStack) {
        (stack.handler)(stack.sig);
    }

    // Push scratch registers
    llvm_asm!("str	    x0, [sp, #-8]!
          str	    x1, [sp, #-8]!
          str	    x2, [sp, #-8]!
          str	    x3, [sp, #-8]!
          str	    x4, [sp, #-8]!
          str	    x5, [sp, #-8]!
          str	    x6, [sp, #-8]!
          str	    x7, [sp, #-8]!
          str	    x8, [sp, #-8]!
          str	    x9, [sp, #-8]!
          str	    x10, [sp, #-8]!
          str	    x11, [sp, #-8]!
          str	    x12, [sp, #-8]!
          str	    x13, [sp, #-8]!
          str	    x14, [sp, #-8]!
          str	    x15, [sp, #-8]!
          str	    x16, [sp, #-8]!
          str	    x17, [sp, #-8]!
          str	    x18, [sp, #-8]!
          str	    x19, [sp, #-8]!
          str	    x20, [sp, #-8]!
          str	    x21, [sp, #-8]!
          str	    x22, [sp, #-8]!
          str	    x23, [sp, #-8]!
          str	    x24, [sp, #-8]!
          str	    x25, [sp, #-8]!
          str	    x26, [sp, #-8]!
          str	    x27, [sp, #-8]!
          str	    x28, [sp, #-8]!"
    : : : : "volatile");

    // Get reference to stack variables
    let sp: usize;
    llvm_asm!("" : "={sp}"(sp) : : : "volatile");

    let ptr = sp as *const SignalHandlerStack;
    let final_lr = (*ptr).lr;

    // Call inner rust function
    inner(&*(sp as *const SignalHandlerStack));

    // Pop scratch registers, error code, and return
    llvm_asm!("ldr	    x28, [sp], #8
          ldr	    x27, [sp], #8
          ldr	    x26, [sp], #8
          ldr	    x25, [sp], #8
          ldr	    x24, [sp], #8
          ldr	    x23, [sp], #8
          ldr	    x22, [sp], #8
          ldr	    x21, [sp], #8
          ldr	    x20, [sp], #8
          ldr	    x19, [sp], #8
          ldr	    x18, [sp], #8
          ldr	    x17, [sp], #8
          ldr	    x16, [sp], #8
          ldr	    x15, [sp], #8
          ldr	    x14, [sp], #8
          ldr	    x13, [sp], #8
          ldr	    x12, [sp], #8
          ldr	    x11, [sp], #8
          ldr	    x10, [sp], #8
          ldr	    x9, [sp], #8
          ldr	    x8, [sp], #8
          ldr	    x7, [sp], #8
          ldr	    x6, [sp], #8
          ldr	    x5, [sp], #8
          ldr	    x4, [sp], #8
          ldr	    x3, [sp], #8
          ldr	    x2, [sp], #8
          ldr	    x1, [sp], #8"
    : : : : "volatile");

    llvm_asm!("mov       x30, $0" : : "r"(final_lr) : "memory" : "volatile");
}
