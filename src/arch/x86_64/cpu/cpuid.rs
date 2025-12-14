use core::arch::x86_64::{__cpuid, __cpuid_count, _rdtsc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CpuVendor {
    Intel,
    AMD,
    Hygon,
    Zhaoxin,
    VIA,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
#[allow(unused)]
pub struct CpuIdInfo {
    pub vendor: CpuVendor,
    pub vendor_str: [u8; 12],
    pub brand_len: usize,
    pub brand_str: [u8; 48],
    pub family: u8,
    pub model: u8,
    pub stepping: u8,

    pub max_basic_leaf: u32,
    pub max_ext_leaf: u32,

    pub logical_per_pkg: u32,
    pub smt_width: u32,

    pub phys_addr_bits: u8,
    pub virt_addr_bits: u8,

    pub tsc_hz_via_0x15: Option<u64>,
    pub base_mhz_via_0x16: Option<u32>,

    pub features: Features,
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(unused)]
pub struct Features {
    pub apic: bool,
    pub x2apic: bool,
    pub tsc: bool,
    pub invariant_tsc: bool,
    pub tsc_deadline: bool,

    pub pae: bool,
    pub pat: bool,
    pub pge: bool,
    pub pse: bool,
    pub pse36: bool,
    pub mtrr: bool,
    pub nx: bool,
    pub la57: bool,
    pub smep: bool,
    pub smap: bool,
    pub umip: bool,

    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub sse41: bool,
    pub sse42: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub avx512dq: bool,
    pub avx512ifma: bool,
    pub avx512pf: bool,
    pub avx512er: bool,
    pub avx512cd: bool,
    pub avx512bw: bool,
    pub avx512vl: bool,
    pub avx512vbmi: bool,
    pub fma: bool,
    pub aes: bool,
    pub pclmulqdq: bool,
    pub sha: bool,
    pub rdrand: bool,
    pub rdseed: bool,

    pub bmi1: bool,
    pub bmi2: bool,
    pub lzcnt: bool,
    pub popcnt: bool,
    pub adx: bool,

    pub xsave: bool,
    pub xsaveopt: bool,
    pub xsavec: bool,
    pub xsaves: bool,
    pub osxsave: bool,

    pub clflush: bool,
    pub clflushopt: bool,
    pub clwb: bool,
    pub prefetchw: bool,
    pub monitor_mwait: bool,

    pub vmx: bool,
    pub svm: bool,
    pub hypervisor: bool,
}

#[inline]
fn vendor_from_bytes(s: &[u8; 12]) -> CpuVendor {
    match s {
        b"GenuineIntel" => CpuVendor::Intel,
        b"AuthenticAMD" => CpuVendor::AMD,
        b"HygonGenuine" => CpuVendor::Hygon,
        b"  Shanghai  " => CpuVendor::Zhaoxin,
        b"CentaurHauls" => CpuVendor::VIA,
        _ => CpuVendor::Unknown,
    }
}

#[inline(always)]
fn cpuid(leaf: u32) -> core::arch::x86_64::CpuidResult {
    unsafe { __cpuid(leaf) }
}

#[inline(always)]
fn cpuidc(leaf: u32, sub: u32) -> core::arch::x86_64::CpuidResult {
    unsafe { __cpuid_count(leaf, sub) }
}

#[inline(always)]
fn xgetbv(xcr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("xgetbv", in("ecx") xcr, out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags));
    }
    (hi as u64) << 32 | (lo as u64)
}

pub fn detect() -> CpuIdInfo {
    let l0 = cpuid(0);
    let max_basic = l0.eax;
    let mut vend = [0u8; 12];
    vend[0..4].copy_from_slice(&l0.ebx.to_le_bytes());
    vend[4..8].copy_from_slice(&l0.edx.to_le_bytes());
    vend[8..12].copy_from_slice(&l0.ecx.to_le_bytes());
    let vendor = vendor_from_bytes(&vend);

    let l1 = cpuid(1);

    let (family_eff, model_eff, stepping) = decode_family_model(l1.eax);
    let family = family_eff as u8;
    let model = model_eff as u8;

    let edx = l1.edx;
    let ecx = l1.ecx;
    let apic = (edx & (1 << 9)) != 0;
    let tsc = (edx & (1 << 4)) != 0;
    let pse = (edx & (1 << 3)) != 0;
    let pse36 = (edx & (1 << 17)) != 0;
    let pat = (edx & (1 << 16)) != 0;
    let pge = (edx & (1 << 13)) != 0;
    let mtrr = (edx & (1 << 12)) != 0;
    let sse = (edx & (1 << 25)) != 0;
    let sse2 = (edx & (1 << 26)) != 0;
    let ssse3 = (ecx & (1 << 9)) != 0;
    let sse3 = (ecx & (1 << 0)) != 0;
    let sse41 = (ecx & (1 << 19)) != 0;
    let sse42 = (ecx & (1 << 20)) != 0;
    let fma = (ecx & (1 << 12)) != 0;
    let aes = (ecx & (1 << 25)) != 0;
    let pclmulqdq = (ecx & (1 << 1)) != 0;
    let rdrand = (ecx & (1 << 30)) != 0;
    let xsave = (ecx & (1 << 26)) != 0;
    let osxsave = (ecx & (1 << 27)) != 0;
    let x2apic = (ecx & (1 << 21)) != 0;
    let monitor_mwait = (ecx & (1 << 3)) != 0;
    let vmx = (ecx & (1 << 5)) != 0;

    let l7_0 = if max_basic >= 7 {
        cpuidc(7, 0)
    } else {
        core::arch::x86_64::CpuidResult {
            eax: 0,
            ebx: 0,
            ecx: 0,
            edx: 0,
        }
    };
    let ebx7 = l7_0.ebx;
    let ecx7 = l7_0.ecx;
    let _edx7 = l7_0.edx;

    let bmi1 = (ebx7 & (1 << 3)) != 0;
    let avx2 = (ebx7 & (1 << 5)) != 0;
    let bmi2 = (ebx7 & (1 << 8)) != 0;
    let rdseed = (ebx7 & (1 << 18)) != 0;
    let adx = (ebx7 & (1 << 19)) != 0;
    let smap = (ebx7 & (1 << 20)) != 0;
    let clflushopt = (ebx7 & (1 << 23)) != 0;
    let clwb = (ebx7 & (1 << 24)) != 0;
    let sha = (ebx7 & (1 << 29)) != 0;
    let umip = (ecx7 & (1 << 2)) != 0;
    let _pku = (ecx7 & (1 << 3)) != 0;
    let _ospke = (ecx7 & (1 << 4)) != 0;
    let smep = (ebx7 & (1 << 7)) != 0;
    let tsc_deadline = (ecx & (1 << 24)) != 0;

    let avx512f = (ebx7 & (1 << 16)) != 0;
    let avx512dq = (ebx7 & (1 << 17)) != 0;
    let avx512ifma = (ebx7 & (1 << 21)) != 0;
    let avx512cd = (ebx7 & (1 << 28)) != 0;
    let avx512bw = (ebx7 & (1 << 30)) != 0;
    let avx512vl = (ebx7 & (1 << 31)) != 0;
    let avx512vbmi = (ecx7 & (1 << 1)) != 0;

    let max_ext = cpuid(0x8000_0000).eax;
    let mut brand = [0u8; 48];
    let mut brand_len = 0usize;
    if max_ext >= 0x8000_0004 {
        let l2 = cpuid(0x8000_0002);
        let l3 = cpuid(0x8000_0003);
        let l4 = cpuid(0x8000_0004);
        for (i, reg) in [
            l2.eax, l2.ebx, l2.ecx, l2.edx, l3.eax, l3.ebx, l3.ecx, l3.edx, l4.eax, l4.ebx, l4.ecx,
            l4.edx,
        ]
        .iter()
        .enumerate()
        {
            let b = reg.to_le_bytes();
            brand[i * 4..i * 4 + 4].copy_from_slice(&b);
        }
        brand_len = 48;
        while brand_len > 0 && (brand[brand_len - 1] == 0 || brand[brand_len - 1] == b' ') {
            brand_len -= 1;
        }
    }

    let invariant_tsc = if max_ext >= 0x8000_0007 {
        (cpuid(0x8000_0007).edx & (1 << 8)) != 0
    } else {
        false
    };

    let mut nx = false;
    let mut la57 = false;
    let mut phys_bits = 36u8;
    let mut virt_bits = 48u8;
    let mut lzcnt = false;
    if max_ext >= 0x8000_0001 {
        let e1 = cpuid(0x8000_0001);
        nx = (e1.edx & (1 << 20)) != 0;
        lzcnt = (e1.ecx & (1 << 5)) != 0;
        la57 = (e1.ecx & (1 << 16)) != 0;
    }
    if max_ext >= 0x8000_0008 {
        let e8 = cpuid(0x8000_0008);
        phys_bits = (e8.eax & 0xff) as u8;
        virt_bits = ((e8.eax >> 8) & 0xff) as u8;
    }

    let avx = (ecx & (1 << 28)) != 0;
    let mut avx_os = false;
    let mut avx512_os = false;
    if xsave && osxsave {
        let x = xgetbv(0);
        avx_os = (x & 0b110) == 0b110;
        avx512_os = (x & (1 << 1)) != 0
            && (x & (1 << 2)) != 0
            && (x & (1 << 5)) != 0
            && (x & (1 << 6)) != 0
            && (x & (1 << 7)) != 0;
    }

    let (xsaveopt, xsavec, xsaves) = if xsave && max_basic >= 0x0D {
        let d1 = cpuidc(0x0D, 1);
        (
            (d1.eax & (1 << 0)) != 0,
            (d1.eax & (1 << 1)) != 0,
            (d1.eax & (1 << 3)) != 0,
        )
    } else {
        (false, false, false)
    };

    let mut tsc_hz_via_15: Option<u64> = None;
    if max_basic >= 0x15 {
        let l15 = cpuid(0x15);
        let denom = l15.eax;
        let numer = l15.ebx;
        let crystal = l15.ecx;
        if denom != 0 && numer != 0 && crystal != 0 {
            let hz = (crystal as u64) * (numer as u64) / (denom as u64);
            tsc_hz_via_15 = Some(hz);
        }
    }

    let mut base_mhz_via_16: Option<u32> = None;
    if max_basic >= 0x16 {
        let l16 = cpuid(0x16);
        if l16.eax != 0 {
            base_mhz_via_16 = Some(l16.eax);
        }
    }

    let (smt, logical) = if max_basic >= 0x0B {
        let mut lvl = 0;
        let mut smt = 1u32;
        let mut pkg = 1u32;
        loop {
            let b = cpuidc(0x0B, lvl);
            if b.ebx == 0 || (b.eax & 0x1F) == 0 {
                break;
            }
            let typ = (b.ecx >> 8) & 0xff;
            let cnt = b.eax & 0x1f;
            match typ {
                1 => smt = 1u32 << cnt,
                2 => pkg = 1u32 << cnt,
                _ => {}
            }
            lvl += 1;
        }
        (smt, logical_count_from_leaf1_or(pkg))
    } else {
        (1, logical_count_from_leaf1_or((l1.ebx >> 16) & 0xff))
    };

    let prefetchw = (max_ext >= 0x8000_0001 && (cpuid(0x8000_0001).ecx & (1 << 8)) != 0)
        || (ecx7 & (1 << 0)) != 0;

    let clflush = (edx & (1 << 19)) != 0;
    let clflushopt_ = clflushopt;
    let clwb_ = clwb;

    let svm = max_ext >= 0x8000_0001 && (cpuid(0x8000_0001).ecx & (1 << 2)) != 0;

    let hypervisor = (ecx & (1 << 31)) != 0;

    let pae = (edx & (1 << 6)) != 0;

    let feats = Features {
        apic,
        x2apic,
        tsc,
        invariant_tsc,
        tsc_deadline,

        pae,
        pat,
        pge,
        pse,
        pse36,
        mtrr,
        nx,
        la57,
        smep,
        smap,
        umip,

        sse,
        sse2,
        sse3,
        ssse3,
        sse41,
        sse42,
        avx: avx && avx_os,
        avx2: avx2 && avx_os,
        avx512f: avx512f && avx512_os,
        avx512dq: avx512dq && avx512_os,
        avx512ifma: avx512ifma && avx512_os,
        avx512pf: false,
        avx512er: false,
        avx512cd: avx512cd && avx512_os,
        avx512bw: avx512bw && avx512_os,
        avx512vl: avx512vl && avx512_os,
        avx512vbmi: avx512vbmi && avx512_os,
        fma,
        aes,
        pclmulqdq,
        sha,
        rdrand,
        rdseed,

        bmi1,
        bmi2,
        lzcnt,
        popcnt: (ecx & (1 << 23)) != 0,
        adx,

        xsave,
        xsaveopt,
        xsavec,
        xsaves,
        osxsave,

        clflush,
        clflushopt: clflushopt_,
        clwb: clwb_,
        prefetchw,
        monitor_mwait,

        vmx,
        svm,
        hypervisor,
    };

    CpuIdInfo {
        vendor,
        vendor_str: vend,
        brand_len,
        brand_str: brand,
        family,
        model,
        stepping,
        max_basic_leaf: max_basic,
        max_ext_leaf: max_ext,
        logical_per_pkg: logical,
        smt_width: smt,
        phys_addr_bits: phys_bits,
        virt_addr_bits: virt_bits,
        tsc_hz_via_0x15: tsc_hz_via_15,
        base_mhz_via_0x16: base_mhz_via_16,
        features: feats,
    }
}

#[inline]
fn decode_family_model(eax: u32) -> (u16, u16, u8) {
    let stepping = (eax & 0xF) as u8;
    let model_lo = ((eax >> 4) & 0xF) as u16;
    let family_lo = ((eax >> 8) & 0xF) as u16;
    let ext_model = ((eax >> 16) & 0xF) as u16;
    let ext_family = ((eax >> 20) & 0xFF) as u16;

    // Effective family
    let family_eff = if family_lo == 0xF {
        family_lo + ext_family
    } else {
        family_lo
    };

    // Effective model
    let model_eff = if family_lo == 0x6 || family_lo == 0xF {
        (ext_model << 4) | model_lo
    } else {
        model_lo
    };

    (family_eff, model_eff, stepping)
}

#[inline]
fn logical_count_from_leaf1_or(fallback: u32) -> u32 {
    let l1 = cpuid(1);
    let lcount = (l1.ebx >> 16) & 0xff;
    if lcount != 0 {
        lcount
    } else {
        if fallback != 0 {
            fallback
        } else {
            1
        }
    }
}

#[allow(unused)]
pub fn vendor_string() -> [u8; 12] {
    let l0 = cpuid(0);
    let mut s = [0u8; 12];
    s[0..4].copy_from_slice(&l0.ebx.to_le_bytes());
    s[4..8].copy_from_slice(&l0.edx.to_le_bytes());
    s[8..12].copy_from_slice(&l0.ecx.to_le_bytes());
    s
}

#[allow(unused)]
pub fn rdtsc() -> u64 {
    unsafe { _rdtsc() as u64 }
}

#[allow(unused)]
pub fn avx_usable_now() -> bool {
    let l1 = cpuid(1);
    let avx = (l1.ecx & (1 << 28)) != 0;
    let xsave = (l1.ecx & (1 << 26)) != 0;
    let osxsave = (l1.ecx & (1 << 27)) != 0;
    if !(avx && xsave && osxsave) {
        return false;
    }
    let x = xgetbv(0);
    (x & 0b110) == 0b110
}

#[allow(unused)]
pub fn avx512_usable_now() -> bool {
    let l7 = if cpuid(0).eax >= 7 {
        cpuidc(7, 0)
    } else {
        return false;
    };
    let has = (l7.ebx & (1 << 16)) != 0;
    if !has {
        return false;
    }

    let l1 = cpuid(1);
    if (l1.ecx & (1 << 26)) == 0 || (l1.ecx & (1 << 27)) == 0 {
        return false;
    }
    let x = xgetbv(0);
    (x & (1 << 1)) != 0
        && (x & (1 << 2)) != 0
        && (x & (1 << 5)) != 0
        && (x & (1 << 6)) != 0
        && (x & (1 << 7)) != 0
}
