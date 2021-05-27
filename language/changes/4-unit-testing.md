# Unit Testing

* Status: Implemented in Move 1.3

## Introduction

Unit testing is a new feature in Move version 1.3 to allow Move programmers
to more easily unit test individual functions. Previously, the only way to test Move
code was using more heavyweight expected-value and transaction-based tests
via the Move CLI and functional test frameworks. Additionally, it was hard
to setup arbitrary state for testing, and to test non-public functions.

## Motivation

Being able to test code thoroughly is an essential ability when developing
Move code. However, without a unit testing framework for Move, achieving
high test coverage for both public and private functions and that is
clear and easily maintainable is a challenging task.

Move unit tests are written in the module whose functionality they
are testing. Because of this, unit tests have access to private functions,
and can freely construct and publish instances of types defined in the
module. This allows a more direct style of testing as compared to
previously, where a complex sequences of transactions may be needed to setup the
global state in such a manner to test a particular code path in a function.

While having the ability construct and publish data in the module that you
are writing the tests in is a an improvement, sometimes, there are
cross-module data dependencies, functions, or structures that you
would like to have for testing purposes alone. To address this need, the
unit testing feature for Move allows expressing that modules, and module members
(`use`s, functions, and structures) are for testing purposes only. When
members are annotated as `#[test_only]` they will not appear in the
compiled bytecode outside of testing. For those familiar with Rust this is
simialr to `#[cfg(test)]`.

## Description

Unit testing for Move adds three new annotations to the language:

* `#[test]`
* `#[test_only]`, and
* `#[expected_failure]`.

They respectively mark a function as a test, mark a module or module member (`use`,
function, or struct) as code to be included for testing only, and mark that
a test is expected to fail (abort).

### Testing Annotations: Their Meaning and Usage

Both the `#[test]` and `#[expected_failure]` annotations can be used
without arguments, and also with arguments.

Without arguments, the `#[test]` annotation can only be placed on a
function with no parameters. This annotation simply marks this function as
a test to be run by the unit testing harness.

```
#[test] // OK!
fun this_is_a_test() { ... }

#[test] // Not right/will fail since the test takes an argument
fun this_is_not_correct(arg: signer) { ... }
```

A test can also be annotated as an `#[expected_failure]`. This annotation
marks that the annotated test should abort -- however the exact abort code is not
checked. The test will be marked as failing only if the test fails with a
non-abort error, or does not abort.

```
#[test]
#[expected_failure]
fun this_test_should_abort_and_pass() { abort 0 }

#[test, expected_failure] // Can have multiple in one attribute with no issue. This test will pass.
fun this_other_test_should_abort_and_pass() { abort 0 }
```

With arguments, a test annotation takes the form `#[test(param_name1 = <address>, ..., param_namen = <address>)]`.
If a function is annotated in such a manner, the function's
parameters must be precisely the parameters `param_name1, ..., param_namen`,
with each parameter having a type of `signer`.

```
#[test(arg = @0xC0FFEE)] // OK!
fun this_is_correct_now(arg: signer) { ... }

#[test(wrong_arg_name = @0xC0FFEE)] // Not correct, the arg name(s) in the attribute need to match the arg names in the test function exactly
fun this_is_incorrect(arg: signer) { ... }

#[test(a = @0xC0FFEE, b = @0xCAFE)] // OK! We support multiple signer arguments, but you must always provide a value for that argument
fun this_works(a: signer, b: signer) { ... }
```

An expected failure annotation can also take the form `#[expected_failure(abort_code = <u64>)]`
If a test function is annotated in such a way, the test must abort with an
abort code equal to `<u64>`. Any other failure or abort code will result in
a test failure.

```
#[test, expected_failure(abort_code = 1)] // This test will fail since it aborts with 0 and we expect it to abort with 0
fun this_test_should_abort_and_fail() { abort 0 }

#[test] // Can also separate them out into multiple attributes with no issue
#[expected_failure(abort_code = 0)] // This test will pass
fun this_test_should_abort_and_pass_too() { abort 0 }
```

A module and any of its members can be declared as test only, in such a
case the item will only be included in the compiled Move bytecode when
compiled in test mode. Additionally, when compiled outside of test mode,
and any non-test `use`s of a `#[test_only]` module will raise an error when
compiled.

```
#[test_only]
module ABC { ... }

#[test_only] // test only attributes can be attached to uses
use 0x1::SomeOtherModule;

#[test_only] // test only attributes can also be attached to structs
struct SomeStruct { ... }

#[test_only] // and functions as well. These can only be called from test code, but are not a test themselves.
fun test_only_function(...) { ... }
```

### Compilation

Move code can be compiled with test-code included by passing the `--test`
flag to the compiler. When compiled outside of test mode, all testing code
will be removed from the code being compiled. You should generally not be
manually passing this flag, but instead letting the unit testing framework
handle this for you.

### Running Unit Tests

Unit tests can be compiled and run by the `move-unit-test` crate. It is designed to work
out-of-the box, so all you will need to start running tests is pass the
files you wish to test (and all dependencies) to it:

```
$ cargo run --bin move-unit-test <move_file_or_dir_containing_move_modules_1> ... <move_file_or_dir_containing_move_modules_n>
```

When running tests, every test will either `PASS`, `FAIL`, or `TIMEOUT`. If a
a testcase fails, if possible the location of the failure along with the
function name that caused the failure will be reported.

A test will be marked as timing out if it exceeds the maximum number of
instructions that can be executed for any single test. This bound can be
changed using the options below. Additionally, while the result of a test
is always deterministic, tests are run in parallel by default, so the
ordering of test results is non-deterministic unless running with only one
thread (see `OPTIONS` below).

There are also a number of options that can be passed as well to fine-tune
the testing experience, and to help debug failing tests. These are:

```
FLAGS:
        --stackless         Use the stackless bytecode interpreter to run the tests and cross check its results with the
                            execution result from Move VM
    -g, --state_on_error    Show the storage state at the end of execution of a failing test
    -h, --help              Prints help information
    -l, --list              List all tests
    -s, --statistics        Report test statistics at the end of testing
    -V, --version           Prints version information
    -v, --verbose           Verbose mode

OPTIONS:
    -f, --filter <filter>                A filter string to determine which unit tests to run
    -i, --instructions <instructions>    Bound the number of Move bytecode instructions that can be executed by any one test [default: 5000]
    -t, --threads <num_threads>          Number of threads to use for running tests [default: 8]

ARGS:
    <sources>...    Source files
```

While each of these flags and options are fairly self-explanatory, it is
worth mentioning that when filtering tests with the `-f` option, any test whose fully qualified
name (i.e., `address::module::function_name`) contains the `<filter>` string
will be run.

## Examples

A simple module using some of the unit testing features is shown in the
following example:

```
// MyModule.move
module 0x1::MyModule {

    struct MyCoin has key { value: u64 }

    public fun make_sure_non_zero_coin(coin: MyCoin): MyCoin {
        assert(coin.value > 0, 0);
        coin
    }

    public fun has_coin(addr: address): bool {
        exists<MyCoin>(addr)
    }

    #[test]
    fun make_sure_non_zero_coin_passes() {
        let coin = MyCoin { value: 1 };
        let MyCoin { value: _ } = make_sure_non_zero_coin(coin);
    }

    #[test]
    // Or #[expected_failure] if we don't care about the abort code
    #[expected_failure(abort_code = 0)]
    fun make_sure_zero_coin_fails() {
        let coin = MyCoin { value: 0 };
        let MyCoin { value: _ } = make_sure_non_zero_coin(coin);
    }

    #[test_only] // test only helper function
    fun publish_coin(account: &signer) {
        move_to(account, MyCoin { value: 1 })
    }

    #[test(a = @0x1, b = @0x2)]
    fun test_has_coin(a: signer, b: signer) {
        publish_coin(&a);
        publish_coin(&b);
        assert(has_coin(@0x1), 0);
        assert(has_coin(@0x2), 1);
        assert(!has_coin(@0x3), 1);
    }
}
```

### Running Tests

```
$ cargo run --bin move-unit-test MyModule.move
Running Move unit tests
[ PASS    ] 0x1::MyModule::make_sure_non_zero_coin_passes
[ PASS    ] 0x1::MyModule::make_sure_zero_coin_fails
[ PASS    ] 0x1::MyModule::test_has_coin
Test result: OK. Total tests: 3; passed: 3; failed: 0
```

### Using Test Flags

#### `-f <str>` or `--filter <str>`

This will only run tests whose fully qualified name contains `<str>`. For example if we wanted to only run tests with `"zero_coin"` in their name:

```
$ cargo run --bin move-unit-test MyModule.move -f zero_coin
Running Move unit tests
[ PASS    ] 0x1::MyModule::make_sure_non_zero_coin_passes
[ PASS    ] 0x1::MyModule::make_sure_zero_coin_fails
Test result: OK. Total tests: 2; passed: 2; failed: 0
```

#### `-i <bound>` or `--instructions <bound>`

This bounds the number of instructions that can be executed for any one test to `<bound>`:
```
cargo run --bin move-unit-test <dir> -i 0 MyModule.move
Running Move unit tests
[ TIMEOUT ] 0x1::MyModule::make_sure_non_zero_coin_passes
[ TIMEOUT ] 0x1::MyModule::make_sure_zero_coin_fails
[ TIMEOUT ] 0x1::MyModule::test_has_coin

Test failures:

Failures in 0x1::MyModule:

┌── make_sure_non_zero_coin_passes ──────
│ Test timed out
└──────────────────


┌── make_sure_zero_coin_fails ──────
│ Test timed out
└──────────────────


┌── test_has_coin ──────
│ Test timed out
└──────────────────

Test result: FAILED. Total tests: 3; passed: 0; failed: 3
```

#### `-s` or `--statistics`

With these flags you can gather statistics about the tests run and report
the runtime and instructions executed for each test. For example, if we
wanted to see the statistics in our `MyModule` example above:

```
$ cargo run --bin move-unit-test MyModule.move -s
Running tests
[ PASS    ] 0x1::MyModule::make_sure_non_zero_coin_passes
[ PASS    ] 0x1::MyModule::make_sure_zero_coin_fails
[ PASS    ] 0x1::MyModule::test_has_coin

Test Statistics:

┌───────────────────────────────────────────────┬────────────┬───────────────────────────┐
│                   Test Name                   │    Time    │   Instructions Executed   │
├───────────────────────────────────────────────┼────────────┼───────────────────────────┤
│ 0x1::MyModule::make_sure_non_zero_coin_passes │   0.005    │             1             │
├───────────────────────────────────────────────┼────────────┼───────────────────────────┤
│ 0x1::MyModule::make_sure_zero_coin_fails      │   0.003    │             1             │
├───────────────────────────────────────────────┼────────────┼───────────────────────────┤
│ 0x1::MyModule::test_has_coin                  │   0.004    │             1             │
└───────────────────────────────────────────────┴────────────┴───────────────────────────┘

Test result: OK. Total tests: 3; passed: 3; failed: 0
```

####  `-g` or `--state-on-error`

These flags will print the global state for any test failures. e.g., if we added the following test to the `MyModule` example:

```
module 0x1::MyModule {
    ...
    #[test(a = @0x1)]
    fun test_has_coin_bad(a: signer) {
        publish_coin(&a);
        assert(has_coin(@0x1), 0);
        assert(has_coin(@0x2), 1);
    }
}
```

we would get get the following output:

```
$ cargo run --bin move-unit-test MyModule.move -g
Running tests
[ PASS    ] 0x1::MyModule::make_sure_non_zero_coin_passes
[ PASS    ] 0x1::MyModule::make_sure_zero_coin_fails
[ PASS    ] 0x1::MyModule::test_has_coin
[ FAIL    ] 0x1::MyModule::test_has_coin_bad

Test failures:

Failures in 0x1::MyModule:

┌── test_has_coin_bad ──────
│ error:
│
│     ┌── MyModule.move:46:9 ───
│     │
│  46 │         assert(has_coin(@0x2), 1);
│     │         ^^^^^^^^^^^^^^^^^^^^^^^^^ Test was not expected to abort but it aborted with 1 here
│     ·
│  43 │     fun test_has_coin_bad(a: signer) {
│     │         ----------------- In this function in 0x1::MyModule
│     │
│
│
│ ────── Storage state at point of failure ──────
│ 0x1:
│       => key 0x1::MyModule::MyCoin {
│           value: 1
│       }
│
└──────────────────

Test result: FAILED. Total tests: 4; passed: 3; failed: 1

```


## Alternatives

* Functional testing
* Expected value tests
* `testmodules`
* More magical alternatives

