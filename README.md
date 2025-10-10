# Zephyrus Contracts

To install dependencies, it is recommended to use [Nix with Flakes](https://github.com/DeterminateSystems/nix-installer).

To enter the development shell with dependencies installed:

```bash
nix develop
```

> **Tip**
> The above command will put you in a bash shell by default.  
> Use `nix develop -c <your-fancy-shell>` if you want to keep your shell.

To build contracts and generate bindings:

```bash
just
```

To view all Just recipes:

```bash
just menu
```

## Introduction

Zephyrus is a vote aggregator for Hydro, built around a maritime theme.  
Each Vessel represents a lockup on Hydro.  
Zephyrus allows a user to either delegate their Vessels and claim a share of the rewards earned by Zephyrus, or to vote directly themselves if they choose.

## Hydro

https://hydro.markets/  
Hydro is an advanced liquidity management solution for the Interchain, designed to transform liquidity allocation in the Cosmos ecosystem. Hydro introduces an innovative way to vote for projects using staked ATOM while still earning staking APR. Built to support and secure the growing ecosystem, Hydro allows users to lock their stakded ATOM to gain Voting Power, empowering them to choose which projects should receive liquidity and how much.

Each Zephyrus Vessel corresponds to a lockup on Hydro. On Hydro, Zephyrus is the sole voter; Zephyrus then manages hydromancers and allows its users to claim their share of rewards.

## Rewards distribution

Zephyrus aims to provide the fairest possible distribution of rewards.  
If a user votes directly, they receive the rewards from their vote minus the protocol’s commission.

When a user delegates their vessels to a hydromancer, they receive a share of the hydromancer’s rewards proportional to the voting power of their vessels, minus both the protocol commission and the hydromancer’s commission. This applies only to proposals where the bid duration is less than or equal to the lock duration of those vessels.

For example, if a user has a vessel locked for 1 round and delegates it to a hydromancer who voted on two proposals—one requiring liquidity to be locked for 2 rounds and another for 1 round—the user will only receive a share of the rewards from the second vote.

Delegated users also receive rewards even if the hydromancer decides not to use their vessels for voting.

To achieve this distribution, Zephyrus strictly tracks the Time-Weighted Shares (TWS) of each vessel. When it is time to distribute rewards, the voting power is calculated based on these TWS and the token ratio information provided by Hydro for the type of token locked in the vessel.
