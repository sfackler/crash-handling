//! `sadness-generator` is a simple crate that provides multiple ways to make
//! any program it is executed in very sad.

#![allow(unsafe_code)]

use std::arch::asm;

/// How you would like your sadness.
#[derive(Copy, Clone, Debug)]
pub enum SadnessFlavor {
    /// `SIGABRT` on Unix.
    ///
    /// This is not implemented on Windows as [`std::process::abort`], the
    /// canonical way to abort processes in Rust, uses the [fastfail](
    /// https://docs.microsoft.com/en-us/cpp/intrinsics/fastfail?view=msvc-170)
    /// intrinsic, which neither raises a `SIGABRT` signal, nor issue a Windows
    /// exception.
    #[cfg(unix)]
    Abort,
    /// * `SIGSEGV` on Linux
    /// * `EXCEPTION_ACCESS_VIOLATION` on Windows
    /// * `EXC_BAD_ACCESS` on Macos
    Segfault,
    /// * `SIGFPE` on Linux
    /// * `EXCEPTION_INT_DIVIDE_BY_ZERO` on Windows
    /// * `EXC_ARITHMETIC` on Macos
    DivideByZero,
    /// * `SIGILL` on Linux
    /// * `EXCEPTION_ILLEGAL_INSTRUCTION` on Windows
    /// * `EXC_BAD_INSTRUCTION` on Macos
    Illegal,
    /// * `SIGBUS` on Linux
    /// * `EXC_BAD_ACCESS` on Macos
    #[cfg(unix)]
    Bus,
    /// * `SIGTRAP` on Linux
    /// * `EXCEPTION_BREAKPOINT` on Windows
    /// * `EXC_BREAKPOINT` on Macos
    Trap,
    /// * `SIGSEGV` on Linux
    /// * `EXCEPTION_STACK_OVERFLOW` on Windows
    /// * `EXC_BAD_ACCESS` on Macos
    StackOverflow {
        /// Raises the signal/exception from a non-[`std::thread::Thread`]
        non_rust_thread: bool,
        /// If using a native thread and there is a signal handler that longjumps,
        /// we can't wait on the thread as we would normally as it would deadlock
        long_jumps: bool,
    },
    /// Raises a [purecall](https://docs.microsoft.com/en-us/cpp/c-runtime-library/reference/purecall?view=msvc-170)
    /// exception
    #[cfg(windows)]
    Purecall,
    /// Raises a [invalid parameter](https://docs.microsoft.com/en-us/cpp/c-runtime-library/reference/invalid-parameter-functions?view=msvc-170)
    /// exception
    #[cfg(windows)]
    InvalidParameter,
}

impl SadnessFlavor {
    /// This only ends one way. Sadness.
    ///
    /// # Safety
    ///
    /// This is not safe. It intentionally crashes.
    pub unsafe fn make_sad(self) -> ! {
        match self {
            #[cfg(unix)]
            Self::Abort => raise_abort(),
            Self::Segfault => raise_segfault(),
            Self::DivideByZero => raise_floating_point_exception(),
            Self::Illegal => raise_illegal_instruction(),
            #[cfg(unix)]
            Self::Bus => raise_bus(),
            Self::Trap => raise_trap(),
            #[allow(unused_variables)]
            Self::StackOverflow {
                non_rust_thread,
                long_jumps,
            } => {
                if !non_rust_thread {
                    raise_stack_overflow()
                } else {
                    #[cfg(unix)]
                    {
                        raise_stack_overflow_in_non_rust_thread(long_jumps)
                    }
                    #[cfg(windows)]
                    {
                        raise_stack_overflow()
                    }
                }
            }
            #[cfg(windows)]
            Self::Purecall => raise_purecall(),
            #[cfg(windows)]
            Self::InvalidParameter => raise_invalid_parameter(),
        }
    }
}

/// [`SadnessFlavor::Abort`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[cfg(unix)]
pub unsafe fn raise_abort() -> ! {
    std::process::abort()
}

/// [`SadnessFlavor::Segfault`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
pub unsafe fn raise_segfault() -> ! {
    let bad_ptr: *mut u8 = 0x42 as _;
    std::ptr::write_volatile(bad_ptr, 1);

    // If we actually get here that means the address is mapped and writable
    // by the current process which is...unexpected
    std::process::abort()
}

/// [`SadnessFlavor::DivideByZero`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
pub unsafe fn raise_floating_point_exception() -> ! {
    let ohno = {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            let mut divisor: u32;
            asm!(
                "mov eax, 1",
                "cdq",
                "mov {div:e}, 0",
                "idiv {div:e}",
                div = out(reg) divisor
            );
            divisor
        }
        #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
        {
            // Unfortunately ARM by default will not raise SIGFPE on divide
            // by 0 and just return 0, so we just explicitly raise here for now
            libc::raise(libc::SIGFPE);
            0
        }
    };

    println!("we won't get here because we've raised a floating point exception: {ohno}");

    std::process::abort()
}

/// [`SadnessFlavor::Illegal`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
pub unsafe fn raise_illegal_instruction() -> ! {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    asm!("ud2");
    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    asm!("udf #0");

    std::process::abort()
}

/// [`SadnessFlavor::Bus`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[cfg(unix)]
pub unsafe fn raise_bus() -> ! {
    let mut temp_name = [0; 14];
    temp_name.copy_from_slice(b"sigbus.XXXXXX\0");

    let bus_fd = libc::mkstemp(temp_name.as_mut_ptr().cast());
    assert!(bus_fd != -1);

    let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;

    let mapping = std::slice::from_raw_parts_mut(
        libc::mmap(
            std::ptr::null_mut(),
            128,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            bus_fd,
            0,
        )
        .cast::<u8>(),
        page_size + page_size / 2,
    );

    libc::unlink(temp_name.as_ptr().cast());

    // https://pubs.opengroup.org/onlinepubs/9699919799/functions/mmap.html
    // The system shall always zero-fill any partial page at the end of
    // an object. Further, the system shall never write out any modified
    // portions of the last page of an object which are beyond its end.
    // References within the address range starting at pa and continuing
    // for len bytes to whole pages following the end of an object shall
    // result in delivery of a SIGBUS signal.
    mapping[20] = 20;
    println!("{}", mapping[20]);

    std::process::abort()
}

/// [`SadnessFlavor::Trap`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
pub unsafe fn raise_trap() -> ! {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    asm!("int3");
    #[cfg(target_arch = "arm")]
    asm!(".inst 0xe7f001f0");
    #[cfg(target_arch = "aarch64")]
    asm!(".inst 0xd4200000");

    std::process::abort()
}

/// [`SadnessFlavor::StackOverflow`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
pub unsafe fn raise_stack_overflow() -> ! {
    fn recurse(data: u64) -> u64 {
        let mut buff = [0u8; 256];
        let mut result = data;

        buff[..9].copy_from_slice(b"junk data");

        for c in buff {
            result += c as u64;
        }

        if result == 0 {
            result
        } else {
            recurse(result) + 1
        }
    }

    recurse(42);
    std::process::abort()
}

/// [`SadnessFlavor::StackOverflow`]
///
/// This is raised inside of a non-Rust `std::thread::Thread` to ensure that
/// alternate stacks apply to all threads, even ones not created from Rust
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[cfg(unix)]
pub unsafe fn raise_stack_overflow_in_non_rust_thread(uses_longjmp: bool) -> ! {
    let mut native: libc::pthread_t = std::mem::zeroed();
    let mut attr: libc::pthread_attr_t = std::mem::zeroed();

    assert_eq!(
        libc::pthread_attr_setstacksize(&mut attr, 2 * 1024 * 1024),
        0,
        "failed to set thread stack size",
    );

    use std::sync;

    let pair = sync::Arc::new((sync::Mutex::new(false), sync::Condvar::new()));
    let tpair = pair.clone();

    extern "C" fn thread_start(arg: *mut libc::c_void) -> *mut libc::c_void {
        {
            let tpair =
                unsafe { sync::Arc::from_raw(arg as *const (sync::Mutex<bool>, sync::Condvar)) };
            let (lock, cvar) = &*tpair;
            let mut started = lock.lock().unwrap();
            *started = true;
            cvar.notify_one();
        }

        unsafe { raise_stack_overflow() };
    }

    let ret = libc::pthread_create(
        &mut native,
        &attr,
        thread_start,
        sync::Arc::into_raw(tpair) as *mut _,
    );

    // We might not get here, but that's ok
    assert_eq!(
        libc::pthread_attr_destroy(&mut attr),
        0,
        "failed to destroy thread attributes"
    );
    assert_eq!(ret, 0, "pthread_create failed");

    // Note if we're doing longjmp shenanigans, we can't do thread join, that
    // has to be handled by the calling code
    if !uses_longjmp {
        assert_eq!(
            libc::pthread_join(native, std::ptr::null_mut()),
            0,
            "failed to join"
        );
    }

    let (lock, cvar) = &*pair;
    let mut started = lock.lock().unwrap();
    while !*started {
        started = cvar.wait(started).unwrap();
    }

    std::thread::sleep(std::time::Duration::from_millis(10));

    #[allow(clippy::empty_loop)]
    loop {}
}

/// [`SadnessFlavor::StackOverflow`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[inline]
#[cfg(unix)]
pub unsafe fn raise_stack_overflow_in_non_rust_thread_normal() -> ! {
    raise_stack_overflow_in_non_rust_thread(false)
}

/// [`SadnessFlavor::StackOverflow`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[inline]
#[cfg(unix)]
pub unsafe fn raise_stack_overflow_in_non_rust_thread_longjmp() -> ! {
    raise_stack_overflow_in_non_rust_thread(true)
}

/// [`SadnessFlavor::Purecall`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[cfg(target_os = "windows")]
pub unsafe fn raise_purecall() -> ! {
    asm!("call _purecall");
    std::process::abort()
}

/// [`SadnessFlavor::InvalidParameter`]
///
/// # Safety
///
/// This is not safe. It intentionally crashes.
#[cfg(target_os = "windows")]
pub unsafe fn raise_invalid_parameter() -> ! {
    extern "C" {
        fn _mbscmp(s1: *const u8, s2: *const u8) -> i32;
    }

    _mbscmp(std::ptr::null(), std::ptr::null());
    std::process::abort()
}
