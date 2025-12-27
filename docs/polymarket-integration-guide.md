# Decoding Polymarket's On-Chain Order Data

**Source:** https://yzc.me/x01Crypto/decoding-polymarket
**Author:** Zichao Yang
**Saved:** December 26, 2024
**Purpose:** Technical reference for Phase 2+ Polymarket integration

---

## I. The On-Chain Foundation: Essential Background Knowledge

### Key Smart Contracts on Polygon Network

| Contract | Address | Purpose |
|----------|---------|---------|
| Conditional Tokens Framework (CTF) | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` | Core Gnosis engine for ERC-1155 outcome tokens |
| CTF Exchange | `0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E` | Binary market trade settlement |
| NegRisk_CTFExchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` | Multi-outcome market exchange |
| NegRiskAdapter | `0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296` | Binary component adapter for complex markets |
| USDC.e (Collateral) | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` | Bridged stablecoin for betting |
| UMA Oracle V2 | `0x6A9D222616C90FcA5754cd1333cFD9b7fb6a4F74` | Market outcome resolution |
| Gnosis Safe Proxy Factory | `0xaacfeea03eb1561c4e67d661e40682bd20e3541b` | MetaMask wallet proxy creation |
| Polymarket Proxy Factory | `0xaB45c5A4B0c941a2F231C04C3f49182e1A254052` | MagicLink wallet proxy creation |

**Event Emission Pattern:**
- `CTF Exchange` / `NegRisk_CTFExchange` → `OrderFilled`, `OrdersMatched`
- `CTF` and `NegRiskAdapter` → `PositionsSplit`, `PositionsMerge`
- `NegRiskAdapter` → `PositionsConverted`

### The Lifecycle of Outcome Tokens

#### Single-Outcome Markets
Binary YES/NO prediction markets (e.g., "Will Candidate A win?") create paired outcome tokens backed by 1 USDC collateral. Each token pair has a unique `positionId`.

#### Multi-Outcome Markets
Markets with multiple possibilities (e.g., "Who will win?") allow bettors to purchase YES/NO tokens for each candidate. Winners redeem their matching tokens for 1 USDC/token.

### Token Operations

**Token Minting (Creation):**
When opposing bettors meet, their combined USDC is locked and new outcome token pairs are minted. Recorded in:
- Single-outcome: `PositionSplit` event from CTF
- Multi-outcome: `PositionSplit` from both CTF and NegRiskAdapter

**Token Matching:**
Existing tokens are traded between bettors. Recorded in `OrderFilled` and `OrdersMatched` events.

**Token Burning (Merge):**
Opposing outcome token pairs are matched and destroyed, releasing collateral. Recorded in:
- Single-outcome: `PositionsMerge` event from CTF
- Multi-outcome: `PositionsMerge` from both CTF and NegRiskAdapter

**Token Conversion:**
Multi-outcome markets allow atomic portfolio transformation for capital efficiency. A trader holding "NO A" and "NO B" tokens can convert to "YES C" plus 1 USDC collateral (economically equivalent). Recorded in `PositionsConverted` event only.

---

## II. Accessing the Data: Your On-Chain Toolkit

### Direct RPC Method with Python

#### 1. Connect to Polygon RPC

```python
from web3 import Web3
from web3.middleware import ExtraDataToPOAMiddleware

POLYGON_RPC_URL = "https://polygon-rpc.com/"
web3 = Web3(Web3.HTTPProvider(POLYGON_RPC_URL))
web3.middleware_onion.inject(ExtraDataToPOAMiddleware, layer=0)

block_number = 51866068
TARGET_ADDRESS = "0xc5d563a36ae78145c45a50134d48A1215220f80a"

logs = web3.eth.get_logs({
    'fromBlock': block_number,
    'toBlock': block_number,
    'address': Web3.to_checksum_address(TARGET_ADDRESS)
})

print(logs)
```

#### 2. Decode with ABI

**Create Event Signature Mapping:**

```python
from web3 import Web3
import json

w3 = Web3()

with open('Polymarket_NegRisk_CTFExchange_abi.json', 'r') as abi_file:
    contract_abi = json.load(abi_file)

contract_address = '0xC5d563A36AE78145C45a50134d48A1215220f80a'
contract_address = Web3.to_checksum_address(contract_address)
contract = w3.eth.contract(address=contract_address, abi=contract_abi)

event_signature_to_event = {}
for abi_item in contract_abi:
    if abi_item['type'] == 'event':
        event_obj = contract.events.__getattr__(abi_item['name'])()
        event_signature = f"{abi_item['name']}({','.join([param['type'] for param in abi_item['inputs']])})"
        signature_hash = Web3.keccak(text=event_signature)
        signature_hash = '0x' + signature_hash.hex()
        event_signature_to_event[signature_hash] = event_obj
```

**Decode Log Data:**

```python
def decode_log(raw_log, event_signature_to_event):
    event_signature = '0x' + raw_log['topics'][0].hex()
    event = event_signature_to_event.get(event_signature)

    log_dict = {
        'address': raw_log['address'],
        'topics': raw_log['topics'],
        'data': raw_log['data'],
        'blockNumber': raw_log['blockNumber'],
        'blockHash': raw_log['blockHash'],
        'transactionHash': raw_log['transactionHash'],
        'transactionIndex': raw_log['transactionIndex'],
        'logIndex': raw_log['logIndex'],
    }

    decoded = event.process_log(log_dict)
    decoded_args = decoded['args']
    decoded_args['event'] = decoded['event']
    return decoded_args

# Apply decoding
decoded_log = decode_log(logs[0], event_signature_to_event)
decoded_log['orderHash'] = '0x' + decoded_log['orderHash'].hex()
print(decoded_log)
```

---

## III. Interpreting the Data: `OrderFilled` and `OrdersMatched` Events

### The `OrderFilled` Event

Emitted for each order (fully or partially) filled.

**Key Fields:**
- `orderHash`: Unique order identifier
- `maker`: Address who placed the limit order
- `taker`: Address filling the order (can be another user or exchange contract)
- `makerAssetId`: Asset maker provides (0 = USDC/buy order; long number = outcome token/sell order)
- `takerAssetId`: Asset taker provides (inverse logic of makerAssetId)
- `makerAmountFilled`: Quantity of maker's asset transferred
- `takerAmountFilled`: Quantity of taker's asset transferred

**Example Interpretation:**

```
{
    'orderHash': '0x83b04dd4f7591c60e21694ce5808587fa5a331bb958994389ce95eddfdb148c6',
    'maker': '0x3Cf3E8d5427aED066a7A5926980600f6C3Cf87B3',
    'taker': '0xd42F6a1634A3707e27cBae14ca966068E5D1047d',
    'makerAssetId': 50315837024432334213827041057729556211989649223066002327303150792784314280840,
    'takerAssetId': 0,
    'makerAmountFilled': 10000000,   // 10 outcome tokens
    'takerAmountFilled': 3300000,   // 3.3 USDC
    'event': 'OrderFilled'
}
```

**Interpretation:** Maker sold 10 outcome tokens; taker bought them for 3.3 USDC.

### The `OrdersMatched` Event

Summary event linking multiple matched orders (one buy, one sell minimum).

```
{
    'takerOrderHash': '0x55a5da3494e8670f67e8952b61ea620bf0939a84065671ba2f4e2930653a7d3c',
    'takerOrderMaker': '0xd42F6a1634A3707e27cBae14ca966068E5D1047d',
    'makerAssetId': 0,
    'takerAssetId': 50315837024432334213827041057729556211989649223066002327303150792784314280840,
    'makerAmountFilled': 3300000,
    'takerAmountFilled': 10000000,
    'event': 'OrdersMatched'
}
```

**Pattern:** Each `OrdersMatched` has at least two corresponding `OrderFilled` events.

---

## IV. Interpreting Position Events

Position-related events are emitted by `NegRiskAdapter` or `CTF` contracts.

### The `PositionSplit` Event

Token pair creation during liquidity provision.

```
{
  'stakeholder': '0xC5d563A36AE78145C45a50134d48A1215220f80a',
  'conditionId': '0x40bbdd26dc08406eedcb913efee7f7faddf50e16fc21caedb4972d57fd71e0d1',
  'amount': 200000000,
  'event': 'PositionSplit'
}
```

### The `PositionsMerge` Event

Token pair destruction when opposite positions are matched.

```
{
  'stakeholder': '0x0d0107a300F01d1786A1C018FB5E75F476184c04',
  'conditionId': '0xbdc8ad55cb8fa7b3c84a971d6056f6a0f354f0f882c3fab598abae19d4e60a5e',
  'amount': 10000000,
  'event': 'PositionsMerge'
}
```

### The `PositionsConverted` Event

Portfolio rebalancing through atomic conversion.

```
{
  'stakeholder': '0xff66A0aDa4122C5d9292Ffb7eC02922d167a7A07',
  'marketId': '0xe3b1bc389210504ebcb9cffe4b0ed06ccac50561e0f24abb6379984cec030f00',
  'indexSet': 1,
  'amount': 10000000,
  'event': 'PositionsConverted'
}
```

**IndexSet Explanation:**

The `indexSet` is a bitmask where each bit position represents an outcome. Bit value 1 means that outcome's "NO" token is supplied as input. Output consists of "YES" tokens for outcomes with 0 bits.

**Formula:** Released collateral = (k - 1) × amount, where k = number of input "NO" tokens.

**Example:** IndexSet = 6 (binary: 0110) means converting 2 "NO" tokens (outcomes 1 and 2). With amount 100:
- Output: 100 "YES" tokens for outcome 0, 100 "YES" tokens for outcome 3
- Released collateral: (2 - 1) × 100 = 100 USDC

---

## V. Real-World Scenarios

### Scenario 1: Simple Trade (Bettor vs. Bettor)

Two opposite-position holders trade directly. Results in two `OrderFilled` events and one `OrdersMatched` event.

**Pattern:** One seller provides outcome tokens (`makerAssetId` ≠ 0); one buyer provides USDC (`takerAssetId` = 0).

### Scenario 2: Minting New Tokens

Two users betting opposite outcomes simultaneously. Both `OrderFilled` events show `makerAssetId` = 0 (both providing USDC). Their combined USDC mints paired tokens.

**Pattern:** Both makers are buyers; exchange creates new token pairs to facilitate.

### Scenario 3: Burning Tokens

Two users holding opposite tokens sell simultaneously. Both `OrderFilled` events show non-zero `makerAssetId` (both providing tokens). Matched tokens burn; collateral releases proportionally.

**Pattern:** Both makers are sellers; matching burns paired tokens and releases locked collateral.

### Scenario 4: Mixed Transactions

Complex matches involving direct trades plus token minting/burning in a single transaction. Requires analyzing all `OrderFilled` events together.

### Scenario 5: Position Conversion

Only visible in `PositionsConverted` events. No direct token trades occur—purely position restructuring maintaining expected value.

---

## Key Takeaways

1. **OrderFilled events reveal individual trade execution** with maker/taker details, assets, and amounts
2. **OrdersMatched events summarize matched order pairs** linking related trades
3. **Position events (Split/Merge/Converted) track token creation/destruction** independent of trading
4. **Event patterns indicate operation type:** minting, burning, trading, or conversion
5. **Complex transactions can involve multiple events** processed sequentially in single blocks

---

## Additional Resources

- **Polygon RPC Providers:** Ankr, Alchemy, Infura (recommended over public endpoints for intensive analysis)
- **Data Indexing Alternative:** The Graph protocol simplifies blockchain queries
- **Contract Verification:** All ABIs available on PolygonScan contract pages

---

## Notes for Calchas Integration

**Key Differences from Kalshi:**
- Polymarket uses on-chain smart contracts (Polygon L2) vs Kalshi's centralized REST API
- Requires Web3/blockchain integration instead of HTTP client
- Event-driven architecture for order monitoring
- USDC.e collateral vs USD fiat
- More complex token mechanics (CTF, splitting, merging, conversion)

**Integration Considerations:**
- Need Polygon RPC endpoint access
- Wallet/key management for transaction signing
- Gas fee estimation and management
- Event listener for real-time market updates
- ABI management for contract interactions

**Phase:** Polymarket integration planned for later (after Kalshi Phase 2-3 complete)
