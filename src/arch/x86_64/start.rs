// Copyright (c) 2019 Stefan Lankes, RWTH Aachen University
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

extern "C" {
    fn main();
}

#[cfg(not(test))]
#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() -> ! {
    main();

    loop {}
}