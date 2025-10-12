"use strict";
/**
 * This file is created and maintained manually.
 */
Object.defineProperty(exports, "__esModule", { value: true });
exports.HydroClient = void 0;
const tx_1 = require("cosmjs-types/cosmwasm/wasm/v1/tx");
const encoding_1 = require("@cosmjs/encoding");
const HydroBase_client_1 = require("./HydroBase.client");
class HydroClient extends HydroBase_client_1.HydroBaseClient {
    constructor(client, sender, contractAddress) {
        super(client, sender, contractAddress);
        this.messageComposer = {
            transferNft: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ transfer_nft: params })),
                    funds: []
                })
            }),
            approve: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ approve: params })),
                    funds: []
                })
            }),
            revoke: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ revoke: params })),
                    funds: []
                })
            }),
            sendNft: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ send_nft: params })),
                    funds: []
                })
            }),
            lockTokensThenSendNft: (params, funds) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ lock_tokens_then_send_nft: params })),
                    funds: [...(funds || [])]
                })
            }),
            lockTokens: (params, funds) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ lock_tokens: params })),
                    funds: [...(funds || [])]
                })
            }),
            refreshLockDuration: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ refresh_lock_duration: params })),
                    funds: []
                })
            }),
            unlockTokens: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ unlock_tokens: params })),
                    funds: []
                })
            }),
            createProposal: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ create_proposal: params })),
                    funds: []
                })
            }),
            vote: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ vote: params })),
                    funds: []
                })
            }),
            unvote: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ unvote: params })),
                    funds: []
                })
            }),
            addAccountToWhitelist: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ add_account_to_whitelist: params })),
                    funds: []
                })
            }),
            removeAccountFromWhitelist: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ remove_account_from_whitelist: params })),
                    funds: []
                })
            }),
            updateConfig: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ update_config: params })),
                    funds: []
                })
            }),
            pause: () => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ pause: {} })),
                    funds: []
                })
            }),
            addTranche: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ add_tranche: params })),
                    funds: []
                })
            }),
            editTranche: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ edit_tranche: params })),
                    funds: []
                })
            }),
            createIcqsForValidators: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ create_icqs_for_validators: params })),
                    funds: []
                })
            }),
            addICQManager: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ add_i_c_q_manager: params })),
                    funds: []
                })
            }),
            removeICQManager: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ remove_i_c_q_manager: params })),
                    funds: []
                })
            }),
            withdrawICQFunds: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ withdraw_i_c_q_funds: params })),
                    funds: []
                })
            }),
            addLiquidityDeployment: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ add_liquidity_deployment: params })),
                    funds: []
                })
            }),
            removeLiquidityDeployment: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ remove_liquidity_deployment: params })),
                    funds: []
                })
            }),
            updateTokenGroupRatio: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ update_token_group_ratio: params })),
                    funds: []
                })
            }),
            addTokenInfoProvider: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ add_token_info_provider: params })),
                    funds: []
                })
            }),
            removeTokenInfoProvider: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ remove_token_info_provider: params })),
                    funds: []
                })
            }),
            deleteConfigs: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ delete_configs: params })),
                    funds: []
                })
            }),
            setGatekeeper: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ set_gatekeeper: params })),
                    funds: []
                })
            }),
            approveAll: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ approve_all: params })),
                    funds: []
                })
            }),
            revokeAll: (params) => ({
                typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
                value: tx_1.MsgExecuteContract.fromPartial({
                    sender: this.sender,
                    contract: this.contractAddress,
                    msg: (0, encoding_1.toUtf8)(JSON.stringify({ revoke_all: params })),
                    funds: []
                })
            })
        };
    }
}
exports.HydroClient = HydroClient;
//# sourceMappingURL=HydroBase.client.msg.js.map