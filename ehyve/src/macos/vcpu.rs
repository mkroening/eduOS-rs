use std;
use consts::*;
use vm::VirtualCPU;
use macos::error::*;
use hypervisor::{vCPU, x86Reg,read_vmx_cap};
use hypervisor::consts::vmcs::*;
use hypervisor;
use x86::bits64::segmentation::*;
use x86::shared::control_regs::*;
use x86::shared::msr::*;
use x86::shared::PrivilegeLevel;

lazy_static! {
	/* read hypervisor enforced capabilities of the machine, (see Intel docs) */
	static ref CAP_PINBASED: u64 = { read_vmx_cap(&hypervisor::VMXCap::PINBASED).unwrap() };
	static ref CAP_PROCBASED: u64 = { read_vmx_cap(&hypervisor::VMXCap::PROCBASED).unwrap() };
	static ref CAP_PROCBASED2: u64 = { read_vmx_cap(&hypervisor::VMXCap::PROCBASED2).unwrap() };
	static ref CAP_ENTRY: u64 = { read_vmx_cap(&hypervisor::VMXCap::ENTRY).unwrap() };
	static ref CAP_EXIT: u64 = { read_vmx_cap(&hypervisor::VMXCap::EXIT).unwrap() };
}

#[derive(Debug)]
pub struct EhyveCPU
{
	id: u32,
	vcpu: vCPU
}

impl EhyveCPU {
    pub fn new(id: u32) -> EhyveCPU {
		EhyveCPU {
			id: id,
			vcpu: vCPU::new().unwrap()
		}
	}

	fn setup_system_gdt(&mut self) -> Result<()> {
		debug!("Setup GDT");

		self.vcpu.write_vmcs(VMCS_GUEST_CS_LIMIT, 0x000fffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_CS_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_CS_AR, 	0xA09B).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_SS_LIMIT, 0x000fffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_SS_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_SS_AR, 0xC093).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_DS_LIMIT, 0x000fffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_DS_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_DS_AR, 0xC093).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_ES_LIMIT, 0x000fffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_ES_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_ES_AR, 0xC093).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_FS_LIMIT, 0x000fffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_FS_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_FS_AR, 0xC093).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_GS_LIMIT, 0x000fffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_GS_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_GS_AR, 0xC093).or_else(to_error)?;

		self.vcpu.write_vmcs(VMCS_GUEST_GDTR_BASE, BOOT_GDT).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_GDTR_LIMIT, ((std::mem::size_of::<u64>() * BOOT_GDT_MAX as usize) - 1) as u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_IDTR_BASE, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_IDTR_LIMIT, 0xffff).or_else(to_error)?;

		self.vcpu.write_vmcs(VMCS_GUEST_TR_LIMIT, 0xffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_TR_AR, 0x8b).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_TR_BASE, 0).or_else(to_error)?;

		self.vcpu.write_vmcs(VMCS_GUEST_LDTR_LIMIT, 0xffff).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_LDTR_AR, 0x82).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_LDTR_BASE, 0).or_else(to_error)?;

		// Reload the segment descriptors
		self.vcpu.write_vmcs(VMCS_GUEST_CS,
			SegmentSelector::new(GDT_KERNEL_CODE as u16, PrivilegeLevel::Ring0).bits() as u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_DS,
			SegmentSelector::new(GDT_KERNEL_DATA as u16, PrivilegeLevel::Ring0).bits() as u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_ES,
			SegmentSelector::new(GDT_KERNEL_DATA as u16, PrivilegeLevel::Ring0).bits() as u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_SS,
			SegmentSelector::new(GDT_KERNEL_DATA as u16, PrivilegeLevel::Ring0).bits() as u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_FS,
			SegmentSelector::new(GDT_KERNEL_DATA as u16, PrivilegeLevel::Ring0).bits() as u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_GS,
			SegmentSelector::new(GDT_KERNEL_DATA as u16, PrivilegeLevel::Ring0).bits() as u64).or_else(to_error)?;

		Ok(())
	}

	fn setup_system_64bit(&mut self) -> Result<()> {
		debug!("Setup 64bit mode");

		/*self.vcpu.write_vmcs(VMCS_CTRL_CR0_MASK, !0u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_CTRL_CR4_MASK, !0u64).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_CTRL_CR4_SHADOW, 0).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_CTRL_CR0_SHADOW, 0).or_else(to_error)?;*/

		let value = CR0_PROTECTED_MODE | CR0_ENABLE_PAGING | CR0_CACHE_DISABLE |
					CR0_NOT_WRITE_THROUGH | CR0_EXTENSION_TYPE;
		self.vcpu.write_vmcs(VMCS_GUEST_CR0, value.bits() as u64).or_else(to_error)?;

		let value = CR4_ENABLE_PAE /*| CR4_ENABLE_PPMC*/;
		self.vcpu.write_vmcs(VMCS_GUEST_CR4, value.bits() as u64).or_else(to_error)?;

		let value = EFER_LME | EFER_LMA;
		self.vcpu.write_vmcs(VMCS_GUEST_IA32_EFER, value).or_else(to_error)?;

		self.vcpu.write_vmcs(VMCS_GUEST_CR3, BOOT_PML4).or_else(to_error)?;
		//self.vcpu.write_vmcs(VMCS_CTRL_CR3_VALUE0, 0x201000).or_else(to_error)?;

		Ok(())
	}

	fn setup_msr(&mut self) -> Result<()> {
		debug!("Enable MSR registers");

		//self.vcpu.enable_native_msr(IA32_EFER, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_FS_BASE, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_GS_BASE, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_KERNEL_GSBASE, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_SYSENTER_CS, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_SYSENTER_EIP, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_SYSENTER_ESP, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_STAR, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_LSTAR, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_CSTAR, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_FMASK, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(TSC, true).or_else(to_error)?;
		self.vcpu.enable_native_msr(IA32_TSC_AUX, true).or_else(to_error)?;

		Ok(())
	}
}

impl VirtualCPU for EhyveCPU {
	fn init(&mut self, entry_point: u64) -> Result<()>
	{

		self.setup_msr()?;

		/*vmx_cap_pinbased = vmx_cap_pinbased | PIN_BASED_INTR | PIN_BASED_NMI | PIN_BASED_VIRTUAL_NMI;
		vmx_cap_pinbased = vmx_cap_pinbased & !PIN_BASED_PREEMPTION_TIMER;
		self.vcpu.write_vmcs(VMCS_CTRL_PIN_BASED, vmx_cap_pinbased).or_else(to_error)?;

		vmx_cap_procbased = vmx_cap_procbased | CPU_BASED_SECONDARY_CTLS | CPU_BASED_MONITOR | CPU_BASED_MWAIT;
		vmx_cap_procbased = vmx_cap_procbased | CPU_BASED_CR8_STORE | CPU_BASED_CR8_LOAD | CPU_BASED_HLT;
		self.vcpu.write_vmcs(VMCS_CTRL_CPU_BASED, vmx_cap_procbased).or_else(to_error)?;

		vmx_cap_procbased2 = vmx_cap_procbased2 | CPU_BASED2_RDTSCP;
		self.vcpu.write_vmcs(VMCS_CTRL_CPU_BASED2, vmx_cap_procbased2).or_else(to_error)?;

		vmx_cap_entry = vmx_cap_entry | VMENTRY_LOAD_EFER;
		self.vcpu.write_vmcs(VMCS_CTRL_VMEXIT_CONTROLS, vmx_cap_entry).or_else(to_error)?;

		vmx_cap_exit = vmx_cap_exit | VMEXIT_HOST_IA32E|VMEXIT_LOAD_EFER;
		self.vcpu.write_vmcs(VMCS_CTRL_VMENTRY_CONTROLS, vmx_cap_exit).or_else(to_error)?;

		self.vcpu.write_vmcs(VMCS_CTRL_EXC_BITMAP, 0xffffffffu64).or_else(to_error)?;

		vmx_cap_pinbased = read_vmx_cap(&hypervisor::VMXCap::PINBASED).unwrap();
		debug!("VMX Pinbased 0x{:x}", vmx_cap_pinbased);
		vmx_cap_procbased = read_vmx_cap(&hypervisor::VMXCap::PROCBASED).unwrap();
		debug!("VMX Procbased 0x{:x}", vmx_cap_procbased);
		vmx_cap_procbased2 = read_vmx_cap(&hypervisor::VMXCap::PROCBASED2).unwrap();
		debug!("VMX Procbased2 0x{:x}", vmx_cap_procbased2);
		vmx_cap_entry = read_vmx_cap(&hypervisor::VMXCap::ENTRY).unwrap();
		debug!("VMX Entry 0x{:x}", vmx_cap_entry);
		vmx_cap_exit = read_vmx_cap(&hypervisor::VMXCap::EXIT).unwrap();
		debug!("VMX Exit 0x{:x}", vmx_cap_exit);*/

		debug!("Setup APIC");
		self.vcpu.set_apic_addr(APIC_DEFAULT_BASE).or_else(to_error)?;

		debug!("Setup instruction pointers");
		self.vcpu.write_vmcs(VMCS_GUEST_RIP, entry_point).or_else(to_error)?;
		self.vcpu.write_vmcs(VMCS_GUEST_RFLAGS, 0x2).or_else(to_error)?;

		self.setup_system_gdt()?;
		self.setup_system_64bit()?;

		Ok(())
	}

	fn run(&mut self) -> Result<()>
	{
		debug!("Run vCPU {}", self.id);
		loop {
			self.vcpu.run().or_else(to_error)?;

			let reason = self.vcpu.read_vmcs(VMCS_RO_EXIT_REASON).unwrap() & 0xffff;
			match reason {
				/*VMX_REASON_VMPTRLD => {
					info!("Handle VMX_REASON_VMPTRLD");
					self.print_registers();
				},*/
				_ => {
					error!("Unhandled exit: 0x{:x}", reason);
					self.print_registers();
					return Err(Error::UnhandledExitReason);
				}
			};
		}

		//Ok(())
	}

	fn print_registers(&self)
	{
		print!("\nDump state of CPU {}\n", self.id);
		print!("\nRegisters:\n");
		print!("----------\n");

		let rip = self.vcpu.read_register(&x86Reg::RIP).unwrap();
		let rflags = self.vcpu.read_register(&x86Reg::RFLAGS).unwrap();
		let rsp = self.vcpu.read_register(&x86Reg::RSP).unwrap();
		let rbp = self.vcpu.read_register(&x86Reg::RBP).unwrap();
		let rax = self.vcpu.read_register(&x86Reg::RAX).unwrap();
		let rbx = self.vcpu.read_register(&x86Reg::RBX).unwrap();
		let rcx = self.vcpu.read_register(&x86Reg::RCX).unwrap();
		let rdx = self.vcpu.read_register(&x86Reg::RDX).unwrap();
		let rsi = self.vcpu.read_register(&x86Reg::RSI).unwrap();
		let rdi = self.vcpu.read_register(&x86Reg::RDI).unwrap();
		let r8 = self.vcpu.read_register(&x86Reg::R8).unwrap();
		let r9 = self.vcpu.read_register(&x86Reg::R9).unwrap();
		let r10 = self.vcpu.read_register(&x86Reg::R10).unwrap();
		let r11 = self.vcpu.read_register(&x86Reg::R11).unwrap();
		let r12 = self.vcpu.read_register(&x86Reg::R12).unwrap();
		let r13 = self.vcpu.read_register(&x86Reg::R13).unwrap();
		let r14 = self.vcpu.read_register(&x86Reg::R14).unwrap();
		let r15 = self.vcpu.read_register(&x86Reg::R15).unwrap();

		print!("rip: {:016x}   rsp: {:016x} flags: {:016x}\n\
			rax: {:016x}   rbx: {:016x}   rcx: {:016x}\n\
			rdx: {:016x}   rsi: {:016x}   rdi: {:016x}\n\
			rbp: {:016x}    r8: {:016x}    r9: {:016x}\n\
			r10: {:016x}   r11: {:016x}   r12: {:016x}\n\
			r13: {:016x}   r14: {:016x}   r15: {:016x}\n",
			rip, rsp, rflags,
			rax, rbx, rcx,
			rdx, rsi, rdi,
			rbp, r8,  r9,
			r10, r11, r12,
			r13, r14, r15);

		let cr0 = self.vcpu.read_register(&x86Reg::CR0).unwrap();
		//let cr1 = self.vcpu.read_register(&x86Reg::CR1).unwrap();
		let cr2 = self.vcpu.read_register(&x86Reg::CR2).unwrap();
		let cr3 = self.vcpu.read_register(&x86Reg::CR3).unwrap();
		let cr4 = self.vcpu.read_register(&x86Reg::CR4).unwrap();
		print!("cr0: {:016x}   cr2: {:016x}   cr3: {:016x}\ncr4: {:016x}\n",
			cr0, cr2, cr3, cr4);

		print!("\nSegment registers:\n");
		print!("------------------\n");
		print!("register  selector  base              limit     type  p dpl db s l g avl\n");

		let cs = self.vcpu.read_register(&x86Reg::CS).unwrap();
		let ds = self.vcpu.read_register(&x86Reg::DS).unwrap();
		let es = self.vcpu.read_register(&x86Reg::ES).unwrap();
		let ss = self.vcpu.read_register(&x86Reg::SS).unwrap();
		let fs = self.vcpu.read_register(&x86Reg::FS).unwrap();
		let gs = self.vcpu.read_register(&x86Reg::GS).unwrap();
		let tr = self.vcpu.read_register(&x86Reg::TR).unwrap();
		let ldtr = self.vcpu.read_register(&x86Reg::LDTR).unwrap();
		let cs_limit = self.vcpu.read_vmcs(VMCS_GUEST_CS_LIMIT).unwrap();
		let cs_base = self.vcpu.read_vmcs(VMCS_GUEST_CS_BASE).unwrap();
		let cs_ar = self.vcpu.read_vmcs(VMCS_GUEST_CS_AR).unwrap();
		let ss_limit = self.vcpu.read_vmcs(VMCS_GUEST_SS_LIMIT).unwrap();
		let ss_base = self.vcpu.read_vmcs(VMCS_GUEST_SS_BASE).unwrap();
		let ss_ar = self.vcpu.read_vmcs(VMCS_GUEST_SS_AR).unwrap();
		let ds_limit = self.vcpu.read_vmcs(VMCS_GUEST_DS_LIMIT).unwrap();
		let ds_base = self.vcpu.read_vmcs(VMCS_GUEST_DS_BASE).unwrap();
		let ds_ar = self.vcpu.read_vmcs(VMCS_GUEST_DS_AR).unwrap();
		let es_limit = self.vcpu.read_vmcs(VMCS_GUEST_ES_LIMIT).unwrap();
		let es_base = self.vcpu.read_vmcs(VMCS_GUEST_ES_BASE).unwrap();
		let es_ar = self.vcpu.read_vmcs(VMCS_GUEST_ES_AR).unwrap();
		let fs_limit = self.vcpu.read_vmcs(VMCS_GUEST_FS_LIMIT).unwrap();
		let fs_base = self.vcpu.read_vmcs(VMCS_GUEST_FS_BASE).unwrap();
		let fs_ar = self.vcpu.read_vmcs(VMCS_GUEST_FS_AR).unwrap();
		let gs_limit = self.vcpu.read_vmcs(VMCS_GUEST_GS_LIMIT).unwrap();
		let gs_base = self.vcpu.read_vmcs(VMCS_GUEST_GS_BASE).unwrap();
		let gs_ar = self.vcpu.read_vmcs(VMCS_GUEST_GS_AR).unwrap();
		let tr_limit = self.vcpu.read_vmcs(VMCS_GUEST_TR_LIMIT).unwrap();
		let tr_base = self.vcpu.read_vmcs(VMCS_GUEST_TR_BASE).unwrap();
		let tr_ar = self.vcpu.read_vmcs(VMCS_GUEST_TR_AR).unwrap();
		let ldtr_limit = self.vcpu.read_vmcs(VMCS_GUEST_LDTR_LIMIT).unwrap();
		let ldtr_base = self.vcpu.read_vmcs(VMCS_GUEST_LDTR_BASE).unwrap();
		let ldtr_ar = self.vcpu.read_vmcs(VMCS_GUEST_LDTR_AR).unwrap();

		println!("cs        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			cs, cs_base, cs_limit, (cs_ar) & 0xf, (cs_ar >> 7) & 0x1, (cs_ar >> 5) & 0x3, (cs_ar >> 14) & 0x1,
			(cs_ar >> 4) & 0x1, (cs_ar >> 13) & 0x1, (cs_ar >> 15) & 0x1, (cs_ar >> 12) & 1);
		println!("ss        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			ss, ss_base, ss_limit, (ss_ar) & 0xf, (ss_ar >> 7) & 0x1, (ss_ar >> 5) & 0x3, (ss_ar >> 14) & 0x1,
			(ss_ar >> 4) & 0x1, (ss_ar >> 13) & 0x1, (ss_ar >> 15) & 0x1, (ss_ar >> 12) & 1);
		println!("ds        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			ds, ds_base, ds_limit, (ds_ar) & 0xf, (ds_ar >> 7) & 0x1, (ds_ar >> 5) & 0x3, (ds_ar >> 14) & 0x1,
			(ds_ar >> 4) & 0x1, (ds_ar >> 13) & 0x1, (ds_ar >> 15) & 0x1, (ds_ar >> 12) & 1);
		println!("es        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			es, es_base, es_limit, (es_ar) & 0xf, (es_ar >> 7) & 0x1, (es_ar >> 5) & 0x3, (es_ar >> 14) & 0x1,
			(es_ar >> 4) & 0x1, (es_ar >> 13) & 0x1, (es_ar >> 15) & 0x1, (es_ar >> 12) & 1);
		println!("fs        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			fs, fs_base, fs_limit, (fs_ar) & 0xf, (fs_ar >> 7) & 0x1, (fs_ar >> 5) & 0x3, (fs_ar >> 14) & 0x1,
			(fs_ar >> 4) & 0x1, (fs_ar >> 13) & 0x1, (fs_ar >> 15) & 0x1, (fs_ar >> 12) & 1);
		println!("gs        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			gs, gs_base, gs_limit, (gs_ar) & 0xf, (gs_ar >> 7) & 0x1, (gs_ar >> 5) & 0x3, (gs_ar >> 14) & 0x1,
			(gs_ar >> 4) & 0x1, (gs_ar >> 13) & 0x1, (gs_ar >> 15) & 0x1, (gs_ar >> 12) & 1);
		println!("tr        {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			tr, tr_base, tr_limit, (tr_ar) & 0xf, (tr_ar >> 7) & 0x1, (tr_ar >> 5) & 0x3, (tr_ar >> 14) & 0x1,
			(tr_ar >> 4) & 0x1, (tr_ar >> 13) & 0x1, (tr_ar >> 15) & 0x1, (tr_ar >> 12) & 1);
		println!("ldt       {:04x}      {:016x}  {:08x}  {:02x}    {:x} {:x}   {:x}  {:x} {:x} {:x} {:x}",
			ldtr, ldtr_base, ldtr_limit, (ldtr_ar) & 0xf, (ldtr_ar >> 7) & 0x1, (ldtr_ar >> 5) & 0x3, (ldtr_ar >> 14) & 0x1,
			(ldtr_ar >> 4) & 0x1, (ldtr_ar >> 13) & 0x1, (ldtr_ar >> 15) & 0x1, (ldtr_ar >> 12) & 1);

		let gdt_base = self.vcpu.read_vmcs(VMCS_GUEST_GDTR_BASE).unwrap();
		let gdt_limit = self.vcpu.read_vmcs(VMCS_GUEST_GDTR_LIMIT).unwrap();
		println!("gdt                 {:016x}  {:08x}", gdt_base, gdt_limit);
		let idt_base = self.vcpu.read_vmcs(VMCS_GUEST_IDTR_BASE).unwrap();
		let idt_limit = self.vcpu.read_vmcs(VMCS_GUEST_IDTR_LIMIT).unwrap();
		println!("idt                 {:016x}  {:08x}", idt_base, idt_limit);

		let efer = self.vcpu.read_vmcs(VMCS_GUEST_IA32_EFER).unwrap();
		print!("\nAPIC:\n");
		print!("-----\n");
		print!("efer: {:016x}  apic base: {:016x}\n", efer, APIC_DEFAULT_BASE);
	}
}

impl Drop for EhyveCPU {
    fn drop(&mut self) {
        debug!("Drop virtual CPU {}", self.id);
		let _ = self.vcpu.destroy();
    }
}
