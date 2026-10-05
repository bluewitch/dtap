# DTap

A Point Of Sale (POS) App for merchants who need relief from Stripe, Square, and centralized encroachment into their margins  tap-mix-pay. A cashier rings a USD amount. The customer pays by QR or an NFC Bolt Card. Settlement runs through BTCPay on hardware the shop controls. Spend keys never sit on that server.

Open source. Rust workspace. Dioxus 0.7 counter. Early: the terminal UI is in tree, the gateway is a stub.

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
