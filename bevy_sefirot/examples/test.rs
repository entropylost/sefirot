use bevy::prelude::*;

#[derive(SystemSet, Copy, Clone, Debug, PartialEq, Eq, Hash)]
struct Test;

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .configure_sets(Update, Test)
        .add_systems(Update, foo.pipe(piped).in_set(Test))
        .add_systems(Update, bar.after(foo));
    for (i, x, y) in app.get_schedule(Update).unwrap().graph().system_sets() {
        println!("{:?} {:?} {:?}", i, x, y.len());
    }
    for (i, x, y) in app.get_schedule(Update).unwrap().graph().systems() {
        println!("{:?} {:?} {:?}", i, x, y.len());
    }
    println!(
        "{:?}",
        app.get_schedule(Update)
            .unwrap()
            .graph()
            .hierarchy()
            .graph()
    );
    println!(
        "{:?}",
        app.get_schedule(Update)
            .unwrap()
            .graph()
            .dependency()
            .graph()
    );
}

fn foo() -> u32 {
    1
}

fn bar() {}

fn piped(_: In<u32>) {}
