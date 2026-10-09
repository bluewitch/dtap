# DTap

A [Point Of Sale (POS)](https://en.wikipedia.org/wiki/Point_of_sale) App for merchants who need relief from [Stripe](https://stripe.com/), [Square](https://squareup.com/us/en), and centralized encroachment into their margins  tap-mix-pay. A cashier rings a $USD amount. The customer pays by QR or an [NFC Bolt Card](https://www.boltcard.org/). Settlement runs through a secured server with [BTCPay Server](https://btcpayserver.org/) and a [full $BTC node](https://bitcoin.org/en/full-node) explorer, on hardware the shop controls. Spend keys never sit on that server.

Open source, under MIT license. Developed with the [Rust Language](https://rust-lang.org/) using [Dioxus 0.7](https://dioxuslabs.com/learn/0.7/) for Web, Desktop, and Mobile. Early: the terminal UI is in tree, the gateway is a stub.  DTap,
at its core, is meant to manage crypto holdings and digital capital, converting and exchanging on-chain, until a 
fiat bridge is required, if it is ever required.

## Architecture

```mermaid
flowchart TB
    subgraph counter ["Counter — Dioxus POS (dtap-app)"]
        T["Terminal keypad / USD"]
        C["Checkout / rails + QR"]
        N["Web NFC Bolt Card lnurlw"]
        X["Exchange lab quote only"]
    end

    subgraph gw ["dtap-gateway — stub"]
        H["/health"]
        CH["/v1/charges"]
        ST["/v1/charges/:id/status"]
        TAP["/v1/tap"]
        Q["/v1/swap/quote"]
    end

    subgraph vault ["BlackVault — financial box"]
        NODE["Full BTC node on volume"]
        PAY["BTCPay Server watch-only"]
        LN["Lightning float only — LND aezeed online"]
    end

    subgraph cold ["Air gap — never on the server"]
        SP["Sparrow signer"]
        METAL["70% black-ice metal seed + separate passphrase"]
    end

    subgraph warmhot ["Operating float"]
        WARM["20% warm device wallets"]
        HOT["10% exchange + channel float"]
    end

    T -->|CreateCharge| CH
    C -->|poll 2s| ST
    N -->|TapRequest| TAP
    X --> Q
    H --> PAY
    CH --> PAY
    ST --> PAY
    TAP -->|pay invoice from card float| LN
    PAY --> NODE
    LN --> NODE
    PAY -.->|xpub / descriptor only| SP
    SP -->|PSBT signed offline| PAY
    METAL --- SP
    LN -->|sweep surplus| SP
    HOT --- LN
    WARM --- SP
