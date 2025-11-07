/**
 * This file is created and maintained manually.
 */

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { MsgExecuteContract } from 'cosmjs-types/cosmwasm/wasm/v1/tx'
import type { Coin } from 'cosmjs-types/cosmos/base/v1beta1/coin'
import { toUtf8 } from '@cosmjs/encoding'
import type { EncodeObject } from '@cosmjs/proto-signing'
import type { ExecuteMsg } from './MarketplaceBase.types'
import { MarketplaceBaseClient, type MarketplaceBaseInterface } from './MarketplaceBase.client'

// Define the type for encoded contract messages
export interface MsgExecuteContractEncodeObject extends EncodeObject {
  readonly typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract'
  readonly value: MsgExecuteContract
}

// Extract parameter types from the base ExecuteMsg type
export type BuyParams = Extract<ExecuteMsg, { buy: any }>['buy']
export type UnlistParams = Extract<ExecuteMsg, { unlist: any }>['unlist']
export type ListParams = Extract<ExecuteMsg, { list: any }>['list']
export type AddOrUpdateCollectionParams = Extract<
  ExecuteMsg,
  { add_or_update_collection: any }
>['add_or_update_collection']
export type RemoveCollectionParams = Extract<
  ExecuteMsg,
  { remove_collection: any }
>['remove_collection']
export type ProposeNewAdminParams = Extract<
  ExecuteMsg,
  { propose_new_admin: any }
>['propose_new_admin']

export interface MarketplaceInterface extends MarketplaceBaseInterface {
  readonly messageComposer: {
    buy: (params: BuyParams, funds: readonly Coin[]) => MsgExecuteContractEncodeObject
    unlist: (params: UnlistParams) => MsgExecuteContractEncodeObject
    list: (params: ListParams) => MsgExecuteContractEncodeObject
    addOrUpdateCollection: (params: AddOrUpdateCollectionParams) => MsgExecuteContractEncodeObject
    removeCollection: (params: RemoveCollectionParams) => MsgExecuteContractEncodeObject
    proposeNewAdmin: (params: ProposeNewAdminParams) => MsgExecuteContractEncodeObject
    claimAdminRole: () => MsgExecuteContractEncodeObject
  }
}

export class MarketplaceClient extends MarketplaceBaseClient implements MarketplaceInterface {
  readonly messageComposer: MarketplaceInterface['messageComposer']

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, sender, contractAddress)

    this.messageComposer = {
      buy: (params: BuyParams, funds: readonly Coin[]): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              buy: {
                collection: params.collection,
                token_id: params.token_id
              }
            })
          ),
          funds: [...(funds || [])]
        })
      }),

      unlist: (params: UnlistParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              unlist: {
                collection: params.collection,
                token_id: params.token_id
              }
            })
          ),
          funds: []
        })
      }),

      list: (params: ListParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              list: {
                collection: params.collection,
                price: params.price,
                token_id: params.token_id
              }
            })
          ),
          funds: []
        })
      }),

      addOrUpdateCollection: (
        params: AddOrUpdateCollectionParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              add_or_update_collection: {
                collection_address: params.collection_address,
                config: params.config
              }
            })
          ),
          funds: []
        })
      }),

      removeCollection: (params: RemoveCollectionParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              remove_collection: {
                collection: params.collection
              }
            })
          ),
          funds: []
        })
      }),

      proposeNewAdmin: (params: ProposeNewAdminParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(
            JSON.stringify({
              propose_new_admin: {
                new_admin: params.new_admin
              }
            })
          ),
          funds: []
        })
      }),

      claimAdminRole: (): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ claim_admin_role: {} })),
          funds: []
        })
      })
    }
  }
}
