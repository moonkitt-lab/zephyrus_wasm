/**
 * This file is created and maintained manually.
 */
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { MsgExecuteContract } from 'cosmjs-types/cosmwasm/wasm/v1/tx';
import type { Coin } from 'cosmjs-types/cosmos/base/v1beta1/coin';
import type { EncodeObject } from '@cosmjs/proto-signing';
import type { ExecuteMsg } from './MarketplaceBase.types';
import { MarketplaceBaseClient, type MarketplaceBaseInterface } from './MarketplaceBase.client';
export interface MsgExecuteContractEncodeObject extends EncodeObject {
    readonly typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract';
    readonly value: MsgExecuteContract;
}
export type BuyParams = Extract<ExecuteMsg, {
    buy: any;
}>['buy'];
export type UnlistParams = Extract<ExecuteMsg, {
    unlist: any;
}>['unlist'];
export type ListParams = Extract<ExecuteMsg, {
    list: any;
}>['list'];
export type AddOrUpdateCollectionParams = Extract<ExecuteMsg, {
    add_or_update_collection: any;
}>['add_or_update_collection'];
export type RemoveCollectionParams = Extract<ExecuteMsg, {
    remove_collection: any;
}>['remove_collection'];
export type ProposeNewAdminParams = Extract<ExecuteMsg, {
    propose_new_admin: any;
}>['propose_new_admin'];
export interface MarketplaceInterface extends MarketplaceBaseInterface {
    readonly messageComposer: {
        buy: (params: BuyParams, funds: readonly Coin[]) => MsgExecuteContractEncodeObject;
        unlist: (params: UnlistParams) => MsgExecuteContractEncodeObject;
        list: (params: ListParams) => MsgExecuteContractEncodeObject;
        addOrUpdateCollection: (params: AddOrUpdateCollectionParams) => MsgExecuteContractEncodeObject;
        removeCollection: (params: RemoveCollectionParams) => MsgExecuteContractEncodeObject;
        proposeNewAdmin: (params: ProposeNewAdminParams) => MsgExecuteContractEncodeObject;
        claimAdminRole: () => MsgExecuteContractEncodeObject;
    };
}
export declare class MarketplaceClient extends MarketplaceBaseClient implements MarketplaceInterface {
    readonly messageComposer: MarketplaceInterface['messageComposer'];
    constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string);
}
//# sourceMappingURL=MarketplaceBase.client.msg.d.ts.map