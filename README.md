# Clawbackable Vesting

A Vesting contract that progressively unlocks a token that can then be distributed.

This is largely forked from [AstroPort](https://github.com/astroport-fi/astroport-core/tree/main/contracts/tokenomics/vesting).

Changes include the optional ability to set vesting info to `clawbackable`, to allow the owner to pull back funds.

---

## InstantiateMsg

Initializes the contract with the address of the vesting token.

```json
{
  "token_addr": "terra..."
}
```

### `receive`

CW20 receive msg.

```json
{
  "receive": {
    "sender": "terra...",
    "amount": "123",
    "msg": "<base64_encoded_json_string>"
  }
}
```

#### `RegisterVestingAccounts`

Creates vesting schedules for the token. Each vesting token should have the Generator contract address as the `VestingContractAddress`. Also, each schedule will unlock tokens at a different rate according to its time duration.

Execute this message by calling the token contract address.

```json
{
  "send": {
    "contract": <VestingContractAddress>,
    "amount": "999",
    "msg": "base64-encodedStringOfWithdrawMsg"
  }
}
```

In `send.msg`, you may encode this JSON string into base64 encoding.

```json
{
  "RegisterVestingAccounts": {
    "vesting_accounts": [
      {
        "address": "terra...",
        "schedules": {
          "start_point": {
            "time": "1634125119000000000",
            "amount": "123"
          },
          "end_point": {
            "time": "1664125119000000000",
            "amount": "123"
          }
        },
        "clawbackable": true
      }
    ]
  }
}
```

### `claim`

Transfer vested tokens from all vesting schedules that have the same `VestingContractAddress` (address that's vesting tokens).

```json
{
  "claim": {
    "recipient": "terra...",
    "amount": "123"
  }
}
```

### `clawback`

Transfer remaining tokens from all vesting schedules that have the same `VestingContractAccress` (address that's vesting tokens), 
to the contract owner.

```json
{
  "clawback": {
    "recipient": "terra..."
  }
}
```

## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `config`

Returns the vesting token contract address.

```json
{
  "config": {}
}
```

### `vesting_account`

Returns all vesting schedules with their details for a specific vesting recipient.

```json
{
  "vesting_account": {
    "address": "terra..."
  }
}
```

### `vesting_accounts`

Returns a paginated list of vesting schedules in chronological order. Given fields are optional.

```json
{
  "vesting_accounts": {
    "start_after": "terra...",
    "limit": 10,
    "order_by": {
      "desc": {}
    }
  }
}
```

### `available amount`

Returns the claimable amount (vested but not yet claimed) of tokens that a vesting target can claim.

```json
{
  "available_amount": {
    "address": "terra..."
  }
}
```