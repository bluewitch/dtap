# DTap architecture

Counter app, gateway, BlackVault, air-gapped signer.

![Architecture](images/architecture.jpg)

Spend keys never sit on the server. BlackVault holds the BTCPay watch-only descriptor. Sparrow signs PSBTs offline.

## Mobile counter

![Terminal](images/ui-terminal.jpg)
![Checkout](images/ui-checkout.jpg)
![Paid](images/ui-paid.jpg)
