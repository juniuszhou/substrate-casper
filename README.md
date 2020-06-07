# substrate-casper

## project overview
The purpose of the project is to show how to introduce a new consensus in substrate.
Casper FFG is a PBFT-inspired and improved consensus protocol. To understand the protocol,
read the https://eips.ethereum.org/EIPS/eip-1011 and https://arxiv.org/abs/1710.09437.
The protocol already implemented in Ethereum, the github repo as https://github.com/ethereum/casper/tree/master/casper/contract.

The benefit of Pow + Casper FFG is that we can finalize the block according to most of validators' vote.
In some context, the finality is critical important to confirm the transaction. It also avoid the miner wasting of resource in the different fork.

Substrate has the most flexible framework and you can use different consensus and integrate it as new module.
To avoid efforts in casper FFG details, the protocol was simplified, for example, the slash not implemented. 
The reward interests is a fixed number, 1 / 10000 defined in the runtime, and reward not be considered as new added deposit.

## components
1. pallet
    
   the Casper FFG protocol implementation, four interfaces are deposit, vote, logout and withdraw.
2. pallet runtime api
   
   provide api for node to access pallet casper. For account, it need the information like minimun deposit, current epoch
   to register as a validator. Then validator need know the source epoch, recommented block hash to submit correct vote.
3. runtime 
   
   runtime need integrate casper pallet module and implement all casper pallet runtime api. 
4. consensus/pow
   
   pow consensus is responsible for block import, block proposal. It also check the new finalized epoch in casper pallet. Pow consensus
   will call apply finalize via client if finalized block found.
5. consensus/sha3pow
   
   implement the hash algorithm for pow, use sha3 to compute the block's hash and verify if hash in new block match the difficult
6. node
    
   application running entity, compose all components and start the program

## future work
The implementation is not industry-ready yet. There are lots of details in protocol to be implemented.
