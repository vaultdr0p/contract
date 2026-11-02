import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}


export const networks = {
  testnet: {
    networkPassphrase: "Test SDF Network ; September 2015",
    contractId: "CB5HTYC4GZW67ET23SACDCWCCDQ3AKU4TYX7SAUQJSKS6LLQLYA4HG53",
  }
} as const

/**
 * Every expected failure mode of the vault.
 * 
 * Codes are part of the contract's public interface and are stable: existing
 * variants must never be renumbered, only appended to.
 */
export const Errors = {
  /**
   * `initialize` was called on an already-initialized vault.
   */
  1: {message:"AlreadyInitialized"},
  /**
   * A function was called before `initialize`.
   */
  2: {message:"NotInitialized"},
  /**
   * An amount was zero or negative.
   */
  3: {message:"InvalidAmount"},
  /**
   * `initialize` was given a deadline at or before the current ledger time.
   */
  4: {message:"InvalidDeadline"},
  /**
   * `register` was called for an address that already has an allocation.
   */
  5: {message:"AllocationExists"},
  /**
   * `claim` or `cancel` was called for an address with no allocation.
   */
  6: {message:"AllocationNotFound"},
  /**
   * `claim` was called on an allocation that was already claimed.
   */
  7: {message:"AlreadyClaimed"},
  /**
   * `claim` was called on an allocation that the admin cancelled.
   */
  8: {message:"AllocationCancelled"},
  /**
   * `register` would exceed the vault balance, or `withdraw` would dip into
   * funds already encumbered by a pending allocation.
   */
  9: {message:"InsufficientFunds"},
  /**
   * `cancel` was called before the deadline.
   */
  10: {message:"DeadlineNotReached"},
  /**
   * `register` or `claim` was called at or after the deadline.
   */
  11: {message:"DeadlinePassed"},
  /**
   * Checked arithmetic over- or underflowed.
   */
  12: {message:"MathOverflow"}
}


/**
 * A single recipient's entitlement.
 */
export interface Allocation {
  amount: i128;
  status: AllocationStatus;
}

/**
 * Lifecycle state of one recipient's allocation.
 * 
 * Allocations are retained after claim or cancel rather than deleted, so that
 * a double claim is distinguishable from a never-registered address.
 */
export type AllocationStatus = {tag: "Pending", values: void} | {tag: "Claimed", values: void} | {tag: "Cancelled", values: void};







export interface Client {
  /**
   * Construct and simulate a fund transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Moves `amount` of the vault token from the admin into the vault.
   * 
   * Permitted after the deadline too: late funds are simply unencumbered and
   * the admin can withdraw them again.
   */
  fund: ({amount}: {amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pays a recipient their own allocation.
   * 
   * Only valid strictly before the deadline. That exclusive bound, paired
   * with `cancel`'s inclusive one, means exactly one of claim and cancel is
   * ever valid — there is no ledger in which they race.
   */
  claim: ({recipient}: {recipient: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Releases an unclaimed allocation after the deadline.
   * 
   * Moves no tokens; it only drops the encumbrance so that `withdraw` can
   * reclaim the funds. Keeping the transfer out means `cancel` cannot fail on
   * a token error, and the admin can cancel many allocations then withdraw
   * once.
   */
  cancel: ({recipient}: {recipient: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Earmarks `amount` for `recipient`.
   * 
   * Rejected unless the new total encumbrance still fits inside the vault's
   * actual balance, which is what makes every pending allocation backed by
   * real tokens. Also rejected for an address that already has an allocation,
   * so an allocation can never silently change under a recipient.
   */
  register: ({recipient, amount}: {recipient: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sends unencumbered funds out of the vault.
   * 
   * Capped at `balance - total_allocated`, so this can never touch tokens
   * owed to a pending allocation. This cap is the contract's central safety
   * property.
   */
  withdraw: ({to, amount}: {to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a available transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Balance not encumbered by any pending allocation: the most the admin
   * could currently `withdraw`.
   */
  available: (options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_admin: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a get_token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_token: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configures the vault. Callable exactly once.
   * 
   * `deadline` is unix seconds and must be in the future: a deadline in the
   * past would create a vault in which nothing could ever be registered or
   * claimed.
   */
  initialize: ({admin, token, deadline}: {admin: string, token: string, deadline: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_deadline transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_deadline: (options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a total_claimed transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  total_claimed: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a get_allocation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_allocation: ({recipient}: {recipient: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Allocation>>>

  /**
   * Construct and simulate a total_allocated transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  total_allocated: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAK1Nb3ZlcyBgYW1vdW50YCBvZiB0aGUgdmF1bHQgdG9rZW4gZnJvbSB0aGUgYWRtaW4gaW50byB0aGUgdmF1bHQuCgpQZXJtaXR0ZWQgYWZ0ZXIgdGhlIGRlYWRsaW5lIHRvbzogbGF0ZSBmdW5kcyBhcmUgc2ltcGx5IHVuZW5jdW1iZXJlZCBhbmQKdGhlIGFkbWluIGNhbiB3aXRoZHJhdyB0aGVtIGFnYWluLgAAAAAAAARmdW5kAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAOtQYXlzIGEgcmVjaXBpZW50IHRoZWlyIG93biBhbGxvY2F0aW9uLgoKT25seSB2YWxpZCBzdHJpY3RseSBiZWZvcmUgdGhlIGRlYWRsaW5lLiBUaGF0IGV4Y2x1c2l2ZSBib3VuZCwgcGFpcmVkCndpdGggYGNhbmNlbGAncyBpbmNsdXNpdmUgb25lLCBtZWFucyBleGFjdGx5IG9uZSBvZiBjbGFpbSBhbmQgY2FuY2VsIGlzCmV2ZXIgdmFsaWQg4oCUIHRoZXJlIGlzIG5vIGxlZGdlciBpbiB3aGljaCB0aGV5IHJhY2UuAAAAAAVjbGFpbQAAAAAAAAEAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAARJSZWxlYXNlcyBhbiB1bmNsYWltZWQgYWxsb2NhdGlvbiBhZnRlciB0aGUgZGVhZGxpbmUuCgpNb3ZlcyBubyB0b2tlbnM7IGl0IG9ubHkgZHJvcHMgdGhlIGVuY3VtYnJhbmNlIHNvIHRoYXQgYHdpdGhkcmF3YCBjYW4KcmVjbGFpbSB0aGUgZnVuZHMuIEtlZXBpbmcgdGhlIHRyYW5zZmVyIG91dCBtZWFucyBgY2FuY2VsYCBjYW5ub3QgZmFpbCBvbgphIHRva2VuIGVycm9yLCBhbmQgdGhlIGFkbWluIGNhbiBjYW5jZWwgbWFueSBhbGxvY2F0aW9ucyB0aGVuIHdpdGhkcmF3Cm9uY2UuAAAAAAAGY2FuY2VsAAAAAAABAAAAAAAAAAlyZWNpcGllbnQAAAAAAAATAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAATpFYXJtYXJrcyBgYW1vdW50YCBmb3IgYHJlY2lwaWVudGAuCgpSZWplY3RlZCB1bmxlc3MgdGhlIG5ldyB0b3RhbCBlbmN1bWJyYW5jZSBzdGlsbCBmaXRzIGluc2lkZSB0aGUgdmF1bHQncwphY3R1YWwgYmFsYW5jZSwgd2hpY2ggaXMgd2hhdCBtYWtlcyBldmVyeSBwZW5kaW5nIGFsbG9jYXRpb24gYmFja2VkIGJ5CnJlYWwgdG9rZW5zLiBBbHNvIHJlamVjdGVkIGZvciBhbiBhZGRyZXNzIHRoYXQgYWxyZWFkeSBoYXMgYW4gYWxsb2NhdGlvbiwKc28gYW4gYWxsb2NhdGlvbiBjYW4gbmV2ZXIgc2lsZW50bHkgY2hhbmdlIHVuZGVyIGEgcmVjaXBpZW50LgAAAAAACHJlZ2lzdGVyAAAAAgAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAMNTZW5kcyB1bmVuY3VtYmVyZWQgZnVuZHMgb3V0IG9mIHRoZSB2YXVsdC4KCkNhcHBlZCBhdCBgYmFsYW5jZSAtIHRvdGFsX2FsbG9jYXRlZGAsIHNvIHRoaXMgY2FuIG5ldmVyIHRvdWNoIHRva2Vucwpvd2VkIHRvIGEgcGVuZGluZyBhbGxvY2F0aW9uLiBUaGlzIGNhcCBpcyB0aGUgY29udHJhY3QncyBjZW50cmFsIHNhZmV0eQpwcm9wZXJ0eS4AAAAACHdpdGhkcmF3AAAAAgAAAAAAAAACdG8AAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAGBCYWxhbmNlIG5vdCBlbmN1bWJlcmVkIGJ5IGFueSBwZW5kaW5nIGFsbG9jYXRpb246IHRoZSBtb3N0IHRoZSBhZG1pbgpjb3VsZCBjdXJyZW50bHkgYHdpdGhkcmF3YC4AAAAJYXZhaWxhYmxlAAAAAAAAAAAAAAEAAAPpAAAACwAAAAM=",
        "AAAAAAAAAAAAAAAJZ2V0X2FkbWluAAAAAAAAAAAAAAEAAAPpAAAAEwAAAAM=",
        "AAAAAAAAAAAAAAAJZ2V0X3Rva2VuAAAAAAAAAAAAAAEAAAPpAAAAEwAAAAM=",
        "AAAAAAAAAMVDb25maWd1cmVzIHRoZSB2YXVsdC4gQ2FsbGFibGUgZXhhY3RseSBvbmNlLgoKYGRlYWRsaW5lYCBpcyB1bml4IHNlY29uZHMgYW5kIG11c3QgYmUgaW4gdGhlIGZ1dHVyZTogYSBkZWFkbGluZSBpbiB0aGUKcGFzdCB3b3VsZCBjcmVhdGUgYSB2YXVsdCBpbiB3aGljaCBub3RoaW5nIGNvdWxkIGV2ZXIgYmUgcmVnaXN0ZXJlZCBvcgpjbGFpbWVkLgAAAAAAAAppbml0aWFsaXplAAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAABXRva2VuAAAAAAAAEwAAAAAAAAAIZGVhZGxpbmUAAAAGAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAAAAAAAMZ2V0X2RlYWRsaW5lAAAAAAAAAAEAAAPpAAAABgAAAAM=",
        "AAAAAAAAAAAAAAANdG90YWxfY2xhaW1lZAAAAAAAAAAAAAABAAAACw==",
        "AAAAAAAAAAAAAAAOZ2V0X2FsbG9jYXRpb24AAAAAAAEAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAABAAAD6AAAB9AAAAAKQWxsb2NhdGlvbgAA",
        "AAAAAAAAAAAAAAAPdG90YWxfYWxsb2NhdGVkAAAAAAAAAAABAAAACw==",
        "AAAABAAAAKpFdmVyeSBleHBlY3RlZCBmYWlsdXJlIG1vZGUgb2YgdGhlIHZhdWx0LgoKQ29kZXMgYXJlIHBhcnQgb2YgdGhlIGNvbnRyYWN0J3MgcHVibGljIGludGVyZmFjZSBhbmQgYXJlIHN0YWJsZTogZXhpc3RpbmcKdmFyaWFudHMgbXVzdCBuZXZlciBiZSByZW51bWJlcmVkLCBvbmx5IGFwcGVuZGVkIHRvLgAAAAAAAAAAAAVFcnJvcgAAAAAAAAwAAAA4YGluaXRpYWxpemVgIHdhcyBjYWxsZWQgb24gYW4gYWxyZWFkeS1pbml0aWFsaXplZCB2YXVsdC4AAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAKkEgZnVuY3Rpb24gd2FzIGNhbGxlZCBiZWZvcmUgYGluaXRpYWxpemVgLgAAAAAADk5vdEluaXRpYWxpemVkAAAAAAACAAAAH0FuIGFtb3VudCB3YXMgemVybyBvciBuZWdhdGl2ZS4AAAAADUludmFsaWRBbW91bnQAAAAAAAADAAAAR2Bpbml0aWFsaXplYCB3YXMgZ2l2ZW4gYSBkZWFkbGluZSBhdCBvciBiZWZvcmUgdGhlIGN1cnJlbnQgbGVkZ2VyIHRpbWUuAAAAAA9JbnZhbGlkRGVhZGxpbmUAAAAABAAAAERgcmVnaXN0ZXJgIHdhcyBjYWxsZWQgZm9yIGFuIGFkZHJlc3MgdGhhdCBhbHJlYWR5IGhhcyBhbiBhbGxvY2F0aW9uLgAAABBBbGxvY2F0aW9uRXhpc3RzAAAABQAAAEFgY2xhaW1gIG9yIGBjYW5jZWxgIHdhcyBjYWxsZWQgZm9yIGFuIGFkZHJlc3Mgd2l0aCBubyBhbGxvY2F0aW9uLgAAAAAAABJBbGxvY2F0aW9uTm90Rm91bmQAAAAAAAYAAAA9YGNsYWltYCB3YXMgY2FsbGVkIG9uIGFuIGFsbG9jYXRpb24gdGhhdCB3YXMgYWxyZWFkeSBjbGFpbWVkLgAAAAAAAA5BbHJlYWR5Q2xhaW1lZAAAAAAABwAAAD1gY2xhaW1gIHdhcyBjYWxsZWQgb24gYW4gYWxsb2NhdGlvbiB0aGF0IHRoZSBhZG1pbiBjYW5jZWxsZWQuAAAAAAAAE0FsbG9jYXRpb25DYW5jZWxsZWQAAAAACAAAAHlgcmVnaXN0ZXJgIHdvdWxkIGV4Y2VlZCB0aGUgdmF1bHQgYmFsYW5jZSwgb3IgYHdpdGhkcmF3YCB3b3VsZCBkaXAgaW50bwpmdW5kcyBhbHJlYWR5IGVuY3VtYmVyZWQgYnkgYSBwZW5kaW5nIGFsbG9jYXRpb24uAAAAAAAAEUluc3VmZmljaWVudEZ1bmRzAAAAAAAACQAAAChgY2FuY2VsYCB3YXMgY2FsbGVkIGJlZm9yZSB0aGUgZGVhZGxpbmUuAAAAEkRlYWRsaW5lTm90UmVhY2hlZAAAAAAACgAAADpgcmVnaXN0ZXJgIG9yIGBjbGFpbWAgd2FzIGNhbGxlZCBhdCBvciBhZnRlciB0aGUgZGVhZGxpbmUuAAAAAAAORGVhZGxpbmVQYXNzZWQAAAAAAAsAAAAoQ2hlY2tlZCBhcml0aG1ldGljIG92ZXItIG9yIHVuZGVyZmxvd2VkLgAAAAxNYXRoT3ZlcmZsb3cAAAAM",
        "AAAAAQAAACFBIHNpbmdsZSByZWNpcGllbnQncyBlbnRpdGxlbWVudC4AAAAAAAAAAAAACkFsbG9jYXRpb24AAAAAAAIAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAGc3RhdHVzAAAAAAfQAAAAEEFsbG9jYXRpb25TdGF0dXM=",
        "AAAAAgAAAL5MaWZlY3ljbGUgc3RhdGUgb2Ygb25lIHJlY2lwaWVudCdzIGFsbG9jYXRpb24uCgpBbGxvY2F0aW9ucyBhcmUgcmV0YWluZWQgYWZ0ZXIgY2xhaW0gb3IgY2FuY2VsIHJhdGhlciB0aGFuIGRlbGV0ZWQsIHNvIHRoYXQKYSBkb3VibGUgY2xhaW0gaXMgZGlzdGluZ3Vpc2hhYmxlIGZyb20gYSBuZXZlci1yZWdpc3RlcmVkIGFkZHJlc3MuAAAAAAAAAAAAEEFsbG9jYXRpb25TdGF0dXMAAAADAAAAAAAAACVSZWdpc3RlcmVkIGFuZCBvd2VkIHRvIHRoZSByZWNpcGllbnQuAAAAAAAAB1BlbmRpbmcAAAAAAAAAABpQYWlkIG91dCB0byB0aGUgcmVjaXBpZW50LgAAAAAAB0NsYWltZWQAAAAAAAAAACpDYW5jZWxsZWQgYnkgdGhlIGFkbWluIGFmdGVyIHRoZSBkZWFkbGluZS4AAAAAAAlDYW5jZWxsZWQAAAA=",
        "AAAABQAAAE1FbWl0dGVkIGJ5IGBmdW5kYC4gYGJhbGFuY2VgIGlzIHRoZSB2YXVsdCdzIHRva2VuIGJhbGFuY2UgYWZ0ZXIgdGhlIHRyYW5zZmVyLgAAAAAAAAAAAAAGRnVuZGVkAAAAAAABAAAABmZ1bmRlZAAAAAAAAwAAAAAAAAAEZnJvbQAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAAAAAAdiYWxhbmNlAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAADBFbWl0dGVkIGJ5IGBjbGFpbWAsIGFmdGVyIHRoZSB0b2tlbnMgaGF2ZSBtb3ZlZC4AAAAAAAAAB0NsYWltZWQAAAAAAQAAAAdjbGFpbWVkAAAAAAIAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAg==",
        "AAAABQAAAEZFbWl0dGVkIGJ5IGBjYW5jZWxgLiBObyB0b2tlbnMgbW92ZTsgb25seSB0aGUgZW5jdW1icmFuY2UgaXMgcmVsZWFzZWQuAAAAAAAAAAAACUNhbmNlbGxlZAAAAAAAAAEAAAAJY2FuY2VsbGVkAAAAAAAAAgAAAAAAAAAJcmVjaXBpZW50AAAAAAAAEwAAAAEAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAC",
        "AAAABQAAADNFbWl0dGVkIGJ5IGB3aXRoZHJhd2AsIGFmdGVyIHRoZSB0b2tlbnMgaGF2ZSBtb3ZlZC4AAAAAAAAAAAlXaXRoZHJhd24AAAAAAAABAAAACXdpdGhkcmF3bgAAAAAAAAIAAAAAAAAAAnRvAAAAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAI=",
        "AAAABQAAAExFbWl0dGVkIGJ5IGByZWdpc3RlcmAuIGB0b3RhbF9hbGxvY2F0ZWRgIGlzIHRoZSBlbmN1bWJyYW5jZSBhZnRlciB0aGUgd3JpdGUuAAAAAAAAAApSZWdpc3RlcmVkAAAAAAABAAAACnJlZ2lzdGVyZWQAAAAAAAMAAAAAAAAACXJlY2lwaWVudAAAAAAAABMAAAABAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAAAAAAAA90b3RhbF9hbGxvY2F0ZWQAAAAACwAAAAAAAAAC",
        "AAAABQAAAB5FbWl0dGVkIG9uY2UsIGJ5IGBpbml0aWFsaXplYC4AAAAAAAAAAAALSW5pdGlhbGl6ZWQAAAAAAQAAAAtpbml0aWFsaXplZAAAAAADAAAAAAAAAAVhZG1pbgAAAAAAABMAAAABAAAAAAAAAAV0b2tlbgAAAAAAABMAAAAAAAAAAAAAAAhkZWFkbGluZQAAAAYAAAAAAAAAAg==" ]),
      options
    )
  }
  public readonly fromJSON = {
    fund: this.txFromJSON<Result<void>>,
        claim: this.txFromJSON<Result<void>>,
        cancel: this.txFromJSON<Result<void>>,
        register: this.txFromJSON<Result<void>>,
        withdraw: this.txFromJSON<Result<void>>,
        available: this.txFromJSON<Result<i128>>,
        get_admin: this.txFromJSON<Result<string>>,
        get_token: this.txFromJSON<Result<string>>,
        initialize: this.txFromJSON<Result<void>>,
        get_deadline: this.txFromJSON<Result<u64>>,
        total_claimed: this.txFromJSON<i128>,
        get_allocation: this.txFromJSON<Option<Allocation>>,
        total_allocated: this.txFromJSON<i128>
  }
}