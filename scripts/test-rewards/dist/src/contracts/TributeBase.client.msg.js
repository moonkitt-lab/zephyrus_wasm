"use strict";
/**
 * This file is created and maintained manually.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.TributeClient = void 0;
const tx_1 = require("cosmjs-types/cosmwasm/wasm/v1/tx");
const encoding_1 = require("@cosmjs/encoding");
const TributeBase_client_1 = require("./TributeBase.client");
class TributeClient extends TributeBase_client_1.TributeBaseClient {
    constructor(client, sender, contractAddress) {
        super(client, sender, contractAddress);
        // Initialize the message composer
        this.messageComposer = {
            claimTribute: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        claim_tribute: {
                            round_id: params.round_id,
                            tranche_id: params.tranche_id,
                            tribute_id: params.tribute_id,
                            voter_address: params.voter_address
                        }
                    })),
                    funds: []
                })
            }),
            addTribute: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        add_tribute: {
                            proposal_id: params.proposal_id,
                            round_id: params.round_id,
                            tranche_id: params.tranche_id
                        }
                    })),
                    funds: []
                })
            }),
            refundTribute: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        refund_tribute: {
                            proposal_id: params.proposal_id,
                            round_id: params.round_id,
                            tranche_id: params.tranche_id,
                            tribute_id: params.tribute_id
                        }
                    })),
                    funds: []
                })
            })
        };
    }
}
exports.TributeClient = TributeClient;
//# sourceMappingURL=TributeBase.client.msg.js.map