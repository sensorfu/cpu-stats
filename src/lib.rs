use std::time::Duration;

#[derive(Debug, Copy, Clone, Default)]
pub struct CpuStats {
    /// normal processes executing in user mode
    pub user: Duration,
    /// processes executing in kernel mode
    pub system: Duration,
}

#[cfg(target_os = "macos")]
pub use macos::cpu_stats;

#[cfg(target_os = "macos")]
mod macos {
    use std::io;
    use std::time::Duration;

    use crate::{clock_ticks, CpuStats};

    pub fn cpu_stats() -> io::Result<crate::CpuStats> {
        let host_port = get_host_port();
        let processor_info = get_host_processor_info(host_port)?;
        deallocate_host_port(host_port)?;

        let mut user_total: usize = 0;
        let mut system_total: usize = 0;

        for (user, system, _idle, _nice) in processor_info {
            user_total += user;
            system_total += system;
        }

        let cpu_stats = CpuStats {
            user: Duration::from_secs(user_total as u64) / clock_ticks() as u32,
            system: Duration::from_secs(system_total as u64) / clock_ticks() as u32,
        };

        Ok(cpu_stats)
    }

    fn get_host_port() -> libc::mach_port_t {
        unsafe { libc::mach_host_self() }
    }

    fn deallocate_host_port(name: libc::mach_port_t) -> io::Result<()> {
        let ret = unsafe { mach2::mach_port::mach_port_deallocate(libc::mach_task_self(), name) };
        if ret == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn get_host_processor_info(
        host: libc::mach_port_t,
    ) -> io::Result<Vec<(usize, usize, usize, usize)>> {
        let mut cpu_count: libc::natural_t = 0;
        let mut cpu_info: libc::processor_info_array_t = std::ptr::null_mut();
        let mut cpu_info_count = 0;

        let ret = unsafe {
            libc::host_processor_info(host, 2, &mut cpu_count, &mut cpu_info, &mut cpu_info_count)
        };

        if ret == -1 {
            return Err(io::Error::last_os_error());
        }

        let cpu_info_slice =
            unsafe { std::slice::from_raw_parts(cpu_info, cpu_info_count as usize) };

        let mut array = Vec::new();
        for chunk in cpu_info_slice.chunks(4) {
            array.push((
                chunk[0] as usize,
                chunk[1] as usize,
                chunk[2] as usize,
                chunk[3] as usize,
            ));
        }

        let ret = unsafe {
            libc::vm_deallocate(
                libc::mach_task_self(),
                cpu_info as libc::vm_address_t,
                cpu_info_count as libc::vm_size_t,
            )
        };

        if ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(array)
    }
}

#[cfg(target_os = "linux")]
pub use linux::read_proc_stat_cpu as cpu_stats;

#[cfg(target_os = "linux")]
mod linux {
    use std::io::{self, BufRead, BufReader};
    use std::time::Duration;

    use crate::{clock_ticks, CpuStats};

    pub fn read_proc_stat_cpu() -> io::Result<crate::CpuStats> {
        let mut fd = BufReader::new(std::fs::File::open("/proc/stat")?);

        let mut line = String::new();
        let _len = fd.read_line(&mut line)?;

        let mut stats = CpuStats::default();

        for (i, v) in line.split_ascii_whitespace().enumerate() {
            match i {
                0 => (),
                1 => stats.user = parse_to_duration(v),
                2 => (),
                3 => stats.system = parse_to_duration(v),
                _ => break,
            }
        }

        Ok(stats)
    }

    fn parse_to_duration(v: &str) -> Duration {
        let v = v.parse().unwrap();
        let d1 = Duration::from_secs(v);
        d1 / clock_ticks() as u32
    }
}

pub use clock_ticks::clock_ticks;

mod clock_ticks {
    use std::io;
    use std::sync::Once;

    static mut CLOCK_TICKS: usize = 0;
    static CLOCK_TICKS_INIT: Once = Once::new();

    /// Returns the number of CPU clock ticks per second.
    pub fn clock_ticks() -> usize {
        unsafe {
            CLOCK_TICKS_INIT.call_once(|| {
                CLOCK_TICKS = sysconf_clock_ticks().unwrap();
            });

            CLOCK_TICKS
        }
    }

    fn sysconf_clock_ticks() -> io::Result<usize> {
        let ret = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

        if ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(ret as usize)
    }
}

#[cfg(test)]
mod tests {
    use crate::{clock_ticks, cpu_stats};

    #[test]
    fn test_clock_ticks() {
        let ticks = clock_ticks();
        assert!(ticks > 0);
    }

    #[test]
    fn test_cpu_stats() {
        let stats = cpu_stats().unwrap();
        assert!(!stats.user.is_zero());
        assert!(!stats.system.is_zero());
    }
}
