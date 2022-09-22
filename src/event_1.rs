// macro_rules! define_observer {
//     ($(($event_name:ident, &args: expr)),+) => {
//         pub struct Observer {
//             // $($event_name: Vec<Box<C>>),+,
//         }
//     };
// }

// define_observer!((dsfsdf), (sdfsf));

pub struct C {
    once: bool,
    f: Box<dyn FnMut(usize) + 'static>,
}

pub struct AAA {
    a: Vec<Box<C>>,
}

impl AAA {
    pub fn new() -> Self {
        Self { a: Vec::new() }
    }

    /// Returns a raw pointer for removing listener.
    pub fn on_a<F>(&mut self, f: F, once: bool) -> *const C
    where
        F: FnMut(usize) + 'static,
    {
        let item = Box::new(C {
            once,
            f: Box::new(f),
        });
        let p = &*item as *const C;

        self.a.push(item);
        p
    }

    pub fn un_a(&mut self, p: *const C) {
        let f = self.a.iter().enumerate().find(|(_, v)| {
            let p0 = &***v as *const C;
            std::ptr::eq(p, p0)
        });

        if let Some((i, _)) = f {
            let _ = self.a.remove(i);
        }
    }

    pub fn emit_a(&mut self, value: usize) {
        let removed = self
            .a
            .iter_mut()
            .filter_map(|item| {
                (item.f)(value);

                if item.once {
                    Some(&**item as *const C)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        removed.into_iter().for_each(|p| {
            self.un_a(p);
        });
    }
}

#[cfg(test)]
mod test {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[derive(Debug)]
    struct B(usize);

    // #[test]
    // fn aa() {
    //     let a = B(1);
    //     {
    //         let p = &a as *const B;
    //         let t = &a as *const B;

    //         println!("{:?}", p);
    //         println!("{:?}", t);
    //         println!("{}", t == p);
    //         println!("{}", std::ptr::eq(p, t));
    //     }
    // }

    // #[test]
    // fn qq() {
    //     let p;
    //     {
    //         let mut b = B(1);
    //         p = &b as *const B;
    //         unsafe { std::ptr::write_bytes(&mut b as *mut B, 10, size_of::<B>()) }
    //     }

    //     unsafe {
    //         let b = &*p;
    //         println!("{:?}", b);
    //     }
    // }

    #[test]
    fn test() {
        let b = Rc::new(RefCell::new(B(0)));

        let mut a = AAA::new();

        // a.on_a(
        //     |v| {
        //         println!("{}", v);
        //     },
        //     false,
        // );
        // a.on_a(
        //     |v| {
        //         println!("{}", v + 10);
        //     },
        //     false,
        // );
        // a.on_a(
        //     |v| {
        //         let c = v + 10;
        //         let d = c.wrapping_add(usize::MAX);
        //         println!("{}", d);
        //     },
        //     false,
        // );
        // let b_0 = b.clone();
        // a.on_a(
        //     move |v| {
        //         b_0.borrow_mut().0 += v * 10;
        //     },
        //     false,
        // );
        // let b_0 = b.clone();
        // a.on_a(
        //     move |v| {
        //         b_0.borrow_mut().0 += v * 20;
        //     },
        //     false,
        // );

        // let f = |v: usize| {
        //     println!("ddd {}", v);
        // };
        // a.on_a(f, false);

        let _ = a.on_a(
            |v| {
                println!("once {}", v);
            },
            true,
        );
        let p = a.on_a(
            |v| {
                println!("un {}", v);
            },
            false,
        );
        a.emit_a(5);
        a.un_a(p);
        a.emit_a(15);

        // println!("{}", b.borrow().0);
    }
}
