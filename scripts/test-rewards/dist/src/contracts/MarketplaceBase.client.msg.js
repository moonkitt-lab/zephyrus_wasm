"use strict";
/**
 * This file is created and maintained manually.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.MarketplaceClient = void 0;
const tx_1 = require("cosmjs-types/cosmwasm/wasm/v1/tx");
const encoding_1 = require("@cosmjs/encoding");
const MarketplaceBase_client_1 = require("./MarketplaceBase.client");
class MarketplaceClient extends MarketplaceBase_client_1.MarketplaceBaseClient {
    constructor(client, sender, contractAddress) {
        super(client, sender, contractAddress);
        this.messageComposer = {
            buy: (params, funds) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        buy: {
                            collection: params.collection,
                            token_id: params.token_id
                        }
                    })),
                    funds: [...(funds || [])]
                })
            }),
            unlist: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        unlist: {
                            collection: params.collection,
                            token_id: params.token_id
                        }
                    })),
                    funds: []
                })
            }),
            list: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        list: {
                            collection: params.collection,
                            price: params.price,
                            token_id: params.token_id
                        }
                    })),
                    funds: []
                })
            }),
            addOrUpdateCollection: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        add_or_update_collection: {
                            collection_address: params.collection_address,
                            config: params.config
                        }
                    })),
                    funds: []
                })
            }),
            removeCollection: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        remove_collection: {
                            collection: params.collection
                        }
                    })),
                    funds: []
                })
            }),
            proposeNewAdmin: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({
                        propose_new_admin: {
                            new_admin: params.new_admin
                        }
                    })),
                    funds: []
                })
            }),
            claimAdminRole: () => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ claim_admin_role: {} })),
                    funds: []
                })
            })
        };
    }
}
exports.MarketplaceClient = MarketplaceClient;
//# sourceMappingURL=MarketplaceBase.client.msg.js.map