
extern crate intercom;
use intercom::*;

trait NotComInterface {}

#[com_class(NotComInterface)]
struct S;

#[com_impl]
impl NotComInterface for S {}
