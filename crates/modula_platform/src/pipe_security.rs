/// Windows named-pipe security: creates a pipe with a current-user-only DACL
/// and remote-client rejection. Compiles only on Windows.
#[cfg(windows)]
mod windows_impl {
    use std::ffi::c_void;
    use std::io;

    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FALSE, HANDLE, TRUE},
        Security::{
            AddAccessAllowedAce, GetLengthSid, GetTokenInformation, InitializeAcl,
            InitializeSecurityDescriptor, SetSecurityDescriptorDacl, TokenUser, ACL, ACL_REVISION,
            SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    const GENERIC_ALL: u32 = 0x10000000;
    const SECURITY_DESCRIPTOR_REVISION: u32 = 1;

    /// Creates the first (or only) instance of a named pipe restricted to the
    /// current OS user, with remote-client connections rejected.
    pub fn create_first_pipe_instance(name: &str) -> io::Result<NamedPipeServer> {
        create_secured_pipe(name, true)
    }

    /// Creates a subsequent (non-first) instance of a named pipe with the same
    /// security policy as the first instance.
    pub fn create_next_pipe_instance(name: &str) -> io::Result<NamedPipeServer> {
        create_secured_pipe(name, false)
    }

    /// Creates a named-pipe instance whose DACL grants only the current user,
    /// supplied via `SECURITY_ATTRIBUTES` at creation time. Applying the DACL
    /// afterwards (`SetSecurityInfo`) needs `WRITE_DAC` on the handle, which the
    /// pipe server handle doesn't carry — hence the descriptor goes in up front.
    fn create_secured_pipe(name: &str, first: bool) -> io::Result<NamedPipeServer> {
        // `acl_buf` backs the ACL that `sd` points into; both must stay alive
        // until `create_with_security_attributes_raw` returns.
        let (acl_buf, mut sd) = unsafe { build_current_user_sd()? };
        let mut sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: (&mut *sd) as *mut SECURITY_DESCRIPTOR as *mut c_void,
            bInheritHandle: FALSE,
        };
        let server = unsafe {
            ServerOptions::new()
                .first_pipe_instance(first)
                .reject_remote_clients(true)
                .in_buffer_size(65536)
                .out_buffer_size(65536)
                .create_with_security_attributes_raw(
                    name,
                    (&mut sa) as *mut SECURITY_ATTRIBUTES as *mut c_void,
                )?
        };
        drop(acl_buf);
        Ok(server)
    }

    /// Builds an absolute `SECURITY_DESCRIPTOR` whose DACL grants only the current
    /// user `GENERIC_ALL`. Returns the ACL backing buffer alongside the descriptor
    /// because the descriptor stores a bare pointer into it — the buffer must not
    /// be dropped while the descriptor is in use.
    unsafe fn build_current_user_sd() -> io::Result<(Vec<u8>, Box<SECURITY_DESCRIPTOR>)> {
        // Open the current process token to read the user SID.
        let process = GetCurrentProcess();
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == FALSE {
            return Err(io::Error::last_os_error());
        }
        let _close_token = CloseOnDrop(token);

        // Two-call pattern: first call gets the required buffer size.
        let mut info_len: u32 = 0;
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut info_len);
        let mut info_buf = vec![0u8; info_len as usize];
        if GetTokenInformation(
            token,
            TokenUser,
            info_buf.as_mut_ptr().cast(),
            info_len,
            &mut info_len,
        ) == FALSE
        {
            return Err(io::Error::last_os_error());
        }
        let token_user = &*(info_buf.as_ptr() as *const TOKEN_USER);
        let user_sid = token_user.User.Sid;

        // Compute ACL buffer size: ACL header + one ACCESS_ALLOWED_ACE with the
        // SID embedded. The ACE itself is (8 byte header + 4 byte mask + SID bytes).
        let sid_len = GetLengthSid(user_sid) as usize;
        // sizeof(ACL)=8, sizeof(ACE header+mask)=8, SID replaces the SidStart DWORD.
        let acl_size = (((8 + 8 + sid_len) + 3) & !3) as u32; // align to DWORD
        let mut acl_buf = vec![0u8; acl_size as usize];
        let acl = acl_buf.as_mut_ptr() as *mut ACL;

        if InitializeAcl(acl, acl_size, ACL_REVISION) == FALSE {
            return Err(io::Error::last_os_error());
        }
        if AddAccessAllowedAce(acl, ACL_REVISION, GENERIC_ALL, user_sid) == FALSE {
            return Err(io::Error::last_os_error());
        }

        let mut sd: Box<SECURITY_DESCRIPTOR> = Box::new(std::mem::zeroed());
        let sd_ptr = (&mut *sd) as *mut SECURITY_DESCRIPTOR as *mut c_void;
        if InitializeSecurityDescriptor(sd_ptr, SECURITY_DESCRIPTOR_REVISION) == FALSE {
            return Err(io::Error::last_os_error());
        }
        // Attach the current-user DACL; an empty (non-null) DACL would deny all.
        if SetSecurityDescriptorDacl(sd_ptr, TRUE, acl, FALSE) == FALSE {
            return Err(io::Error::last_os_error());
        }
        Ok((acl_buf, sd))
    }

    struct CloseOnDrop(HANDLE);
    impl Drop for CloseOnDrop {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{create_first_pipe_instance, create_next_pipe_instance};
