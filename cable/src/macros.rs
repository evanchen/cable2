// debug_log!(logname,fmt,var1,var2,...)
#[macro_export]
macro_rules! debug {
    ($outter:expr,$($arg:tt)*) => {
        if $outter.can_log_debug() {
            let str = format!($($arg)*);
            $outter.log("debug",&str);
        }
    };
}

#[macro_export]
macro_rules! warning {
    ($outter:expr,$($arg:tt)*) => {
        if $outter.can_log_warning() {
            let str = format!($($arg)*);
            $outter.log("warn",&str);
        }
    };
}

#[macro_export]
macro_rules! info {
    ($outter:expr,$($arg:tt)*) => {
        if $outter.can_log_info() {
            let str = format!($($arg)*);
            $outter.log("info",&str);
        }
    };
}

#[macro_export]
macro_rules! error {
    ($outter:expr,$($arg:tt)*) => {
        let str = format!($($arg)*);
        $outter.log("error",&str);
    };
}
