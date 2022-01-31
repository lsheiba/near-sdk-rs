# Workspaces Migration Guide
NEAR Simulator was meant to be an in-place replacement of a blockchain environment for the purpose of testing NEAR contracts. However, simulating NEAR ledger turned out to be a much more complex endeavour than was anticipated. Eventually, the idea of workspaces was born - a library for automating workflows and writing tests for NEAR smart contracts using a real NEAR network (localnet, testnet or mainnet). Thus, NEAR Simulator is being deprecated in favor of workspaces. As the two libraries have two vastly different APIs this document was created to ease the migration process for developers.

TODO: I do not have a whole lot of context here why exactly simtests were not suitable for our purposes, so if anyone wants to elaborate the preceding paragraph please do.

## Transitioning existing near-sdk-sim powered tests to workspaces-rs
As an example, let's take a look at transitioning from near-sdk-sim `3.2.0` (the last non-deprecated release) to workspaces `0.2.1`. Given that near-sdk-sim is deprecated, it is very unlikely that its API is going to ever change, but future releases of workspaces-rs might. Hopefully this guide is going to be helpful even if you are migrating your project to a more recent version, but also feel free to migrate your tests to `0.2.1` using this guide first and upgrade to the most recent workspaces-rs version later by looking at the release notes to see how public API has changed since `0.2.1`.

### Async runtime and error handling
In this section we will be working purely with test signatures, so it applies to pretty much all NEAR contract tests regardless of what is written inside. We will walk through each change one by one. Let's start with how your tests look like right now; chances are something like this:

```rust
#[test]
fn test_transfer() {
    ...
}
```

First big change is that workspaces API is asynchronous, meaning that contract function calls return values that implement `Future` trait. You will not be able to operate on the call results in a synchronous environment, thus you will have to add an async runtime (if you do not already have one). In this guide we are going to be using `tokio`, but you should be able to use any other alternative (e.g. async-std, smol). Rewrite the test above like this:

```rust
#[tokio::test]
async fn test_transfer() {
    ...
}
```

NOTE: If you are using another attribute on top of the standard `#[test]`, make sure it plays nicely with the async runtime of your choosing. For example, if you are using [`test-env-log`](https://crates.io/crates/test-env-log) and `tokio`, then you need to mark your tests with `#[test_env_log::test(tokio::test)]`.

The second change is that workspaces makes an extensive use of [`anyhow::Result`](https://docs.rs/anyhow/latest/anyhow/type.Result.html). Although you can work with `Result` directly, our recommendation is to make your tests return `anyhow::Result<()>` like this:

```rust
#[tokio::test]
async fn test_transfer() -> anyhow::Result<()> {
    ...
}
```

This way you can use `?` anywhere inside the test to safely unpack any `anyhow::Result<R>` type to `R` (will be very useful further down the guide). Note that the test will fail if `anyhow::Result<R>` cannot be unpacked.

### Initialization and deploying contracts
Unlike near-sdk-sim, workspaces spins up an actual NEAR node instance and makes all calls through it. First, you need to decide which network you want your tests to be run on:
* sandbox - perfect choice if you are just interested in local development and testing; `workspaces-rs` will instantiate a [sandbox](https://github.com/near/sandbox) instance on your local machine which will run a NEAR node on a local network.
* testnet - an environment much closer to real world, you can test integration with other deployed contracts on testnet without bearing any risk.
* mainnet - a network with reduced amount of features due to how dangerous it can be to do transactions there, but can still be useful for automating deployments and pulling deployed contracts.

In this guide we will be focusing on sandbox since it covers the same use cases near-sdk-sim did. But of course feel free to explore whether other networks can be of potential use to you when writing new tests/workflows. You can find one of the ways to initialize simulator and deploy a contract below (the other way is through `deploy!` macro which we will look at in the next section):

```rust
use near_sdk_sim::{init_simulator, to_yocto};

near_sdk_sim::lazy_static_include::lazy_static_include_bytes! {
    WASM_BYTES => "res/contract.wasm",
}

const ID: &str = "contract-id";

...

let root = init_simulator(...);
let contract = root.deploy(&WASM_BYTES, ID.parse().unwrap(), to_yocto("5"));
```

Although workspaces-rs provides a way to specify the account id for a contract to be deployed, usually it does not matter in the context of a test. If you are fine with generating a random developer account and initializing it with 100N, then you can use replace the snippet above with this:

```rust
let worker = workspaces::sandbox();
let contract = worker.dev_deploy(include_bytes!("../res/contract.wasm")).await?;
```

Alternatively, use this if you care about the account id:

```rust
let worker = workspaces::sandbox();
let (_, sk) = worker.dev_generate().await;
let id: AccountId = "contract-id".parse()?;
let contract = worker
    .create_tla_and_deploy(
        id,
        sk,
        include_bytes!("../examples/res/non_fungible_token.wasm"),
    )
    .await?
    .result;
```

Or, if you want to create a subaccount with a certain balance:

```rust
use near_units::parse_near;

let worker = workspaces::sandbox();
let id: AccountId = "contract-id".parse()?;
let contract = worker
    .root_account()
    .create_subaccount(&worker, id)
    .initial_balance(parse_near!("5 N"))
    .transact()
    .await?
    .result;
```

TODO: Is there a reason why we can't control the initial balance with `dev_deploy`?

### Making transactions and view calls
TMP: Note that unlike `call!` macro from near-sdk-sim, you cannot specify an initialization method for `Worker::dev_deploy`. You will have to call the method yourself using one of the method described in the next section. One other difference is that workspaces is not aware of the contract API, hence you cannot pass the contract struct generated by `#[near_bindgen]`. As you will see in the following section, this makes the API a bit more difficult to use as you are losing the static type checking you were getting with near-sdk-sim. NEAR is aware of this issue, and we are working on making this experience smoother. 

Workspaces have a unified way of making all types of calls via a [builder](https://doc.rust-lang.org/1.0.0/style/ownership/builders.html) pattern. Generally, calls are constructed by following these steps:

1. Create a `CallBuilder` by invoking `Contract::call`
2. Pass function call arguments via `CallBuilder::args_json` or `CallBuilder::args_borsh`
3. Configure gas and deposit (if needed) via `CallBuilder::gas` and `CallBuilder::deposit`
4. Finalize the call by consuming builder via `CallBuilder::transaction` or `CallBuilder::view` depending on what kind of call you want to make

Reference these examples for migrating your own calls:

```rust
/*
 * Example 1: A transaction call with deposit
 */
call!(
    root,
    nft.nft_approve(TOKEN_ID.into(), alice.account_id(), None),
    deposit = 170000000000000000000
);

// `contract` is a previously deployed `Contract`
contract
    .call(&worker, "nft_approve")
    .args_json((TOKEN_ID, alice.id(), Option::<String>::None))?
    // Set prepaid gas to 300 TGas (the max amount) if you do not care
    // about gas usage and just want to test the function logic
    .gas(300_000_000_000_000)
    .deposit(170000000000000000000)
    .transact()
    .await?;

/*
 * Example 2: A view call with a single argument
 */
view!(nft.nft_token(TOKEN_ID.into()));

contract
    .call(&worker, "nft_token")
    .args_json((TOKEN_ID,))?
    .view()
    .await?;
```

Note that you have to pass arguments as any serializable type representing a sequential list. Tuples are usually the best candidate due to their heterogeneous nature (remember that you can construct a unary tuple by placing a comma before the closing bracket like this: `(el,)`).

## Caveats
### Batched transactions
### Custom genesis
### Inspecting logs
