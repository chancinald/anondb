# domacro

Pass the name of a function to a macro for arbitrary expansion. Does not break IDE/auto-complete/syntax highlighting for the function in question!

## Example use

If I have a trait with multiple implementations, I want to write a generic test function and invoke it for all implementations of the trait.

```rs
use domacro::domacro;

trait WorkerTrait {
  fn works(&self) -> bool;
}

#[domacro(all_impls)]
fn test<T: WorkerTrait>(instance: T) {
  assert!(instance.works());
}

#[macro_export]
macro_rules! all_impls {
    ($fn_name:ident) => {
        paste::paste! {
            #[test]
            fn [<aliceworker_ $fn_name>]() {
                $fn_name::<AliceWorker>();
            }

            #[test]
            fn [<bobworker_ $fn_name>]() {
                $fn_name::<BobWorker>();
            }

            #[test]
            fn [<zuluworker_ $fn_name>]() {
                $fn_name::<ZuluWorker>();
            }
        }
    };
}
```
