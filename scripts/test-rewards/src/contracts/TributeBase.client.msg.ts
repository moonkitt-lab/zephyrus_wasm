/**
 * This file is created and maintained manually.
 */

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { MsgExecuteContract } from 'cosmjs-types/cosmwasm/wasm/v1/tx'
import { toUtf8 } from '@cosmjs/encoding'
import type { EncodeObject } from '@cosmjs/proto-signing'
import type { ExecuteMsg } from './TributeBase.types'
import { TributeBaseClient, type TributeBaseInterface } from './TributeBase.client'

// Define the type for encoded contract messages
export interface MsgExecuteContractEncodeObject extends EncodeObject {
  readonly typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract'
  readonly value: MsgExecuteContract
}

// Extract parameter types from the base ExecuteMsg type
export type ClaimTributeParams = Extract<ExecuteMsg, { claim_tribute: any }>['claim_tribute']
export type AddTributeParams = Extract<ExecuteMsg, { add_tribute: any }>['add_tribute']
export type RefundTributeParams = Extract<ExecuteMsg, { refund_tribute: any }>['refund_tribute']

export interface TributeInterface extends TributeBaseInterface {
  // Add a message composer object
  readonly messageComposer: {
    claimTribute: (params: ClaimTributeParams) => MsgExecuteContractEncodeObject
    addTribute: (params: AddTributeParams) => MsgExecuteContractEncodeObject
    refundTribute: (params: RefundTributeParams) => MsgExecuteContractEncodeObject
  }
}

export class TributeClient extends TributeBaseClient implements TributeInterface {
  readonly messageComposer: TributeInterface['messageComposer']

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, sender, contractAddress)

    // Initialize the message composer
    this.messageComposer = {
      claimTribute: (params: ClaimTributeParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              claim_tribute: {
                round_id: params.round_id,
                tranche_id: params.tranche_id,
                tribute_id: params.tribute_id,
                voter_address: params.voter_address
              }
            })
          ),
          funds: []
        })
      }),

      addTribute: (params: AddTributeParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              add_tribute: {
                proposal_id: params.proposal_id,
                round_id: params.round_id,
                tranche_id: params.tranche_id
              }
            })
          ),
          funds: []
        })
      }),

      refundTribute: (params: RefundTributeParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              refund_tribute: {
                proposal_id: params.proposal_id,
                round_id: params.round_id,
                tranche_id: params.tranche_id,
                tribute_id: params.tribute_id
              }
            })
          ),
          funds: []
        })
      })
    }
  }
}