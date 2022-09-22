use std::time::SystemTime;

#[macro_export]
macro_rules! define_observer {
    ($name:ident, $(($event_name:ident, $container_name:ident, ($($y_name:ident: $y:ty),*))),+) => {
        $(
            pub struct $container_name(Vec<Box<(bool, Box<dyn FnMut($($y),*) + 'static>)>>);

            impl $container_name {
                /// Returns a raw pointer for removing listener.
                pub fn on<F>(&mut self, f: F, once: bool) -> *const (bool, Box<dyn FnMut($($y),*) + 'static>)
                where
                    F: FnMut($($y),*) + 'static,
                {
                    let item: Box<(bool, Box<dyn FnMut($($y),*) + 'static>)> = Box::new((once, Box::new(f)));
                    let p = &*item as *const (bool, Box<dyn FnMut($($y),*) + 'static>);

                    self.0.push(item);
                    p
                }

                pub fn un(&mut self, p: *const (bool, Box<dyn FnMut($($y),*) + 'static>)) {
                    let f = self.0.iter().enumerate().find(|(_, v)| {
                        let p0 = &***v as *const (bool, Box<dyn FnMut($($y),*) + 'static>);
                        std::ptr::eq(p, p0)
                    });

                    if let Some((i, _)) = f {
                        let _ = self.0.remove(i);
                    }
                }

                pub fn raise(&mut self, $($y_name: $y),*) {
                    let removed = self
                        .0
                        .iter_mut()
                        .filter_map(|item| {
                            (item.1)($($y_name),*);

                            if item.0 {
                                Some(&**item as *const (bool, Box<dyn FnMut($($y),*) + 'static>))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    removed.into_iter().for_each(|p| {
                        self.un(p);
                    });
                }
            }
        )+

        pub struct $name {
            $(pub $event_name: $container_name),+,
        }

        impl $name {
            pub fn new() -> Self {
                Self {
                    $(
                        $event_name: $container_name(Vec::with_capacity(10))
                    ),+,
                }
            }

        }
    };
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    define_observer!(TestObserver, (opened, Opened, (time: SystemTime)), (closed, Closed, ()));

    #[test]
    fn test() {
        let mut observer = TestObserver::new();
        let p = observer.opened.on(
            |time| {
                println!("{}", time.elapsed().unwrap().as_millis());
            },
            false,
        );

        let time = SystemTime::now();
        std::thread::sleep(Duration::from_secs(1));
        observer.opened.raise(time);

        std::thread::sleep(Duration::from_secs(1));
        observer.opened.raise(time);

        std::thread::sleep(Duration::from_secs(1));
        observer.opened.un(p);
        observer.opened.raise(time);
    }
}

//     #[test]
//     fn test() {
//         let b = Rc::new(RefCell::new(B(0)));

//         let mut aaa = AAA::new();

//         aaa.a.on(
//             |v| {
//                 println!("{}", v);
//             },
//             false,
//         );
//         aaa.a.on(
//             |v| {
//                 println!("{}", v + 10);
//             },
//             false,
//         );
//         aaa.a.on(
//             |v| {
//                 let c = v + 10;
//                 let d = c.wrapping_add(usize::MAX);
//                 println!("{}", d);
//             },
//             false,
//         );
//         let b_0 = b.clone();
//         aaa.a.on(
//             move |v| {
//                 b_0.borrow_mut().0 += v * 10;
//             },
//             false,
//         );
//         let b_0 = b.clone();
//         aaa.a.on(
//             move |v| {
//                 b_0.borrow_mut().0 += v * 20;
//             },
//             false,
//         );

//         let f = |v: usize| {
//             println!("ddd {}", v);
//         };
//         aaa.a.on(f, false);

//         let _ = aaa.a.on(
//             |v| {
//                 println!("once {}", v);
//             },
//             true,
//         );
//         let p = aaa.a.on(
//             |v| {
//                 println!("un {}", v);
//             },
//             false,
//         );
//         aaa.a.raise(5);
//         aaa.a.un(p);
//         aaa.a.raise(15);

//         // println!("{}", b.borrow().0);
//     }
// }
