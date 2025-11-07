/**
 * This file is created and maintained manually.
 */
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate';
import { MsgExecuteContract } from 'cosmjs-types/cosmwasm/wasm/v1/tx';
import type { EncodeObject } from '@cosmjs/proto-signing';
import type { ExecuteMsg } from './TributeBase.types';
import { TributeBaseClient, type TributeBaseInterface } from './TributeBase.client';
export interface MsgExecuteContractEncodeObject extends EncodeObject {
    readonly typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract';
    readonly value: MsgExecuteContract;
}
export type ClaimTributeParams = Extract<ExecuteMsg, {
    claim_tribute: any;
}>['claim_tribute'];
export type AddTributeParams = Extract<ExecuteMsg, {
    add_tribute: any;
}>['add_tribute'];
export type RefundTributeParams = Extract<ExecuteMsg, {
    refund_tribute: any;
}>['refund_tribute'];
export interface TributeInterface extends TributeBaseInterface {
    readonly messageComposer: {
        claimTribute: (params: ClaimTributeParams) => MsgExecuteContractEncodeObject;
        addTribute: (params: AddTributeParams) => MsgExecuteContractEncodeObject;
        refundTribute: (params: RefundTributeParams) => MsgExecuteContractEncodeObject;
    };
}
export declare class TributeClient extends TributeBaseClient implements TributeInterface {
    readonly messageComposer: TributeInterface['messageComposer'];
    constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string);
}
//# sourceMappingURL=TributeBase.client.msg.d.ts.map