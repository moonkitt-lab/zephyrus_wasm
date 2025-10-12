/**
 * This file is created and maintained manually.
 */

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { MsgExecuteContract } from 'cosmjs-types/cosmwasm/wasm/v1/tx'
import type { Coin } from 'cosmjs-types/cosmos/base/v1beta1/coin'
import { toUtf8 } from '@cosmjs/encoding'
import type { EncodeObject } from '@cosmjs/proto-signing'
import type { ExecuteMsg } from './HydroBase.types'
import { HydroBaseClient, type HydroBaseInterface } from './HydroBase.client'

// Define the type for encoded contract messages
export interface MsgExecuteContractEncodeObject extends EncodeObject {
  readonly typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract'
  readonly value: MsgExecuteContract
}

// Extract parameter types from the base ExecuteMsg type
export type TransferNftParams = Extract<ExecuteMsg, { transfer_nft: any }>['transfer_nft']
export type ApproveParams = Extract<ExecuteMsg, { approve: any }>['approve']
export type RevokeParams = Extract<ExecuteMsg, { revoke: any }>['revoke']
export type SendNftParams = Extract<ExecuteMsg, { send_nft: any }>['send_nft']
export type LockTokensThenSendNftParams = Extract<
  ExecuteMsg,
  { lock_tokens_then_send_nft: any }
>['lock_tokens_then_send_nft']
export type LockTokensParams = Extract<ExecuteMsg, { lock_tokens: any }>['lock_tokens']
export type RefreshLockDurationParams = Extract<
  ExecuteMsg,
  { refresh_lock_duration: any }
>['refresh_lock_duration']
export type UnlockTokensParams = Extract<ExecuteMsg, { unlock_tokens: any }>['unlock_tokens']
export type CreateProposalParams = Extract<ExecuteMsg, { create_proposal: any }>['create_proposal']
export type VoteParams = Extract<ExecuteMsg, { vote: any }>['vote']
export type UnvoteParams = Extract<ExecuteMsg, { unvote: any }>['unvote']
export type AddAccountToWhitelistParams = Extract<
  ExecuteMsg,
  { add_account_to_whitelist: any }
>['add_account_to_whitelist']
export type RemoveAccountFromWhitelistParams = Extract<
  ExecuteMsg,
  { remove_account_from_whitelist: any }
>['remove_account_from_whitelist']
export type UpdateConfigParams = Extract<ExecuteMsg, { update_config: any }>['update_config']
export type AddTrancheParams = Extract<ExecuteMsg, { add_tranche: any }>['add_tranche']
export type EditTrancheParams = Extract<ExecuteMsg, { edit_tranche: any }>['edit_tranche']
export type CreateIcqsForValidatorsParams = Extract<
  ExecuteMsg,
  { create_icqs_for_validators: any }
>['create_icqs_for_validators']
export type AddICQManagerParams = Extract<
  ExecuteMsg,
  { add_i_c_q_manager: any }
>['add_i_c_q_manager']
export type RemoveICQManagerParams = Extract<
  ExecuteMsg,
  { remove_i_c_q_manager: any }
>['remove_i_c_q_manager']
export type WithdrawICQFundsParams = Extract<
  ExecuteMsg,
  { withdraw_i_c_q_funds: any }
>['withdraw_i_c_q_funds']
export type AddLiquidityDeploymentParams = Extract<
  ExecuteMsg,
  { add_liquidity_deployment: any }
>['add_liquidity_deployment']
export type RemoveLiquidityDeploymentParams = Extract<
  ExecuteMsg,
  { remove_liquidity_deployment: any }
>['remove_liquidity_deployment']
export type UpdateTokenGroupRatioParams = Extract<
  ExecuteMsg,
  { update_token_group_ratio: any }
>['update_token_group_ratio']
export type AddTokenInfoProviderParams = Extract<
  ExecuteMsg,
  { add_token_info_provider: any }
>['add_token_info_provider']
export type RemoveTokenInfoProviderParams = Extract<
  ExecuteMsg,
  { remove_token_info_provider: any }
>['remove_token_info_provider']
export type DeleteConfigsParams = Extract<ExecuteMsg, { delete_configs: any }>['delete_configs']
export type SetGatekeeperParams = Extract<ExecuteMsg, { set_gatekeeper: any }>['set_gatekeeper']
export type ApproveAllParams = Extract<ExecuteMsg, { approve_all: any }>['approve_all']
export type RevokeAllParams = Extract<ExecuteMsg, { revoke_all: any }>['revoke_all']

export interface HydroInterface extends HydroBaseInterface {
  readonly messageComposer: {
    transferNft: (params: TransferNftParams) => MsgExecuteContractEncodeObject
    approve: (params: ApproveParams) => MsgExecuteContractEncodeObject
    revoke: (params: RevokeParams) => MsgExecuteContractEncodeObject
    sendNft: (params: SendNftParams) => MsgExecuteContractEncodeObject
    lockTokensThenSendNft: (
      params: LockTokensThenSendNftParams,
      funds: readonly Coin[]
    ) => MsgExecuteContractEncodeObject
    lockTokens: (params: LockTokensParams, funds: readonly Coin[]) => MsgExecuteContractEncodeObject
    refreshLockDuration: (params: RefreshLockDurationParams) => MsgExecuteContractEncodeObject
    unlockTokens: (params: UnlockTokensParams) => MsgExecuteContractEncodeObject
    createProposal: (params: CreateProposalParams) => MsgExecuteContractEncodeObject
    vote: (params: VoteParams) => MsgExecuteContractEncodeObject
    unvote: (params: UnvoteParams) => MsgExecuteContractEncodeObject
    addAccountToWhitelist: (params: AddAccountToWhitelistParams) => MsgExecuteContractEncodeObject
    removeAccountFromWhitelist: (
      params: RemoveAccountFromWhitelistParams
    ) => MsgExecuteContractEncodeObject
    updateConfig: (params: UpdateConfigParams) => MsgExecuteContractEncodeObject
    pause: () => MsgExecuteContractEncodeObject
    addTranche: (params: AddTrancheParams) => MsgExecuteContractEncodeObject
    editTranche: (params: EditTrancheParams) => MsgExecuteContractEncodeObject
    createIcqsForValidators: (
      params: CreateIcqsForValidatorsParams
    ) => MsgExecuteContractEncodeObject
    addICQManager: (params: AddICQManagerParams) => MsgExecuteContractEncodeObject
    removeICQManager: (params: RemoveICQManagerParams) => MsgExecuteContractEncodeObject
    withdrawICQFunds: (params: WithdrawICQFundsParams) => MsgExecuteContractEncodeObject
    addLiquidityDeployment: (params: AddLiquidityDeploymentParams) => MsgExecuteContractEncodeObject
    removeLiquidityDeployment: (
      params: RemoveLiquidityDeploymentParams
    ) => MsgExecuteContractEncodeObject
    updateTokenGroupRatio: (params: UpdateTokenGroupRatioParams) => MsgExecuteContractEncodeObject
    addTokenInfoProvider: (params: AddTokenInfoProviderParams) => MsgExecuteContractEncodeObject
    removeTokenInfoProvider: (
      params: RemoveTokenInfoProviderParams
    ) => MsgExecuteContractEncodeObject
    deleteConfigs: (params: DeleteConfigsParams) => MsgExecuteContractEncodeObject
    setGatekeeper: (params: SetGatekeeperParams) => MsgExecuteContractEncodeObject
    approveAll: (params: ApproveAllParams) => MsgExecuteContractEncodeObject
    revokeAll: (params: RevokeAllParams) => MsgExecuteContractEncodeObject
  }
}

export class HydroClient extends HydroBaseClient implements HydroInterface {
  readonly messageComposer: HydroInterface['messageComposer']

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, sender, contractAddress)

    this.messageComposer = {
      transferNft: (params: TransferNftParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ transfer_nft: params })),
          funds: []
        })
      }),

      approve: (params: ApproveParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ approve: params })),
          funds: []
        })
      }),

      revoke: (params: RevokeParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ revoke: params })),
          funds: []
        })
      }),

      sendNft: (params: SendNftParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ send_nft: params })),
          funds: []
        })
      }),

      lockTokensThenSendNft: (
        params: LockTokensThenSendNftParams,
        funds: readonly Coin[]
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ lock_tokens_then_send_nft: params })),
          funds: [...(funds || [])]
        })
      }),

      lockTokens: (
        params: LockTokensParams,
        funds: readonly Coin[]
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ lock_tokens: params })),
          funds: [...(funds || [])]
        })
      }),

      refreshLockDuration: (params: RefreshLockDurationParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ refresh_lock_duration: params })),
          funds: []
        })
      }),

      unlockTokens: (params: UnlockTokensParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ unlock_tokens: params })),
          funds: []
        })
      }),

      createProposal: (params: CreateProposalParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ create_proposal: params })),
          funds: []
        })
      }),

      vote: (params: VoteParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ vote: params })),
          funds: []
        })
      }),

      unvote: (params: UnvoteParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ unvote: params })),
          funds: []
        })
      }),

      addAccountToWhitelist: (
        params: AddAccountToWhitelistParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ add_account_to_whitelist: params })),
          funds: []
        })
      }),

      removeAccountFromWhitelist: (
        params: RemoveAccountFromWhitelistParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ remove_account_from_whitelist: params })),
          funds: []
        })
      }),

      updateConfig: (params: UpdateConfigParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ update_config: params })),
          funds: []
        })
      }),

      pause: (): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ pause: {} })),
          funds: []
        })
      }),

      addTranche: (params: AddTrancheParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ add_tranche: params })),
          funds: []
        })
      }),

      editTranche: (params: EditTrancheParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ edit_tranche: params })),
          funds: []
        })
      }),

      createIcqsForValidators: (
        params: CreateIcqsForValidatorsParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ create_icqs_for_validators: params })),
          funds: []
        })
      }),

      addICQManager: (params: AddICQManagerParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ add_i_c_q_manager: params })),
          funds: []
        })
      }),

      removeICQManager: (params: RemoveICQManagerParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ remove_i_c_q_manager: params })),
          funds: []
        })
      }),

      withdrawICQFunds: (params: WithdrawICQFundsParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ withdraw_i_c_q_funds: params })),
          funds: []
        })
      }),

      addLiquidityDeployment: (
        params: AddLiquidityDeploymentParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ add_liquidity_deployment: params })),
          funds: []
        })
      }),

      removeLiquidityDeployment: (
        params: RemoveLiquidityDeploymentParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ remove_liquidity_deployment: params })),
          funds: []
        })
      }),

      updateTokenGroupRatio: (
        params: UpdateTokenGroupRatioParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ update_token_group_ratio: params })),
          funds: []
        })
      }),

      addTokenInfoProvider: (
        params: AddTokenInfoProviderParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ add_token_info_provider: params })),
          funds: []
        })
      }),

      removeTokenInfoProvider: (
        params: RemoveTokenInfoProviderParams
      ): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ remove_token_info_provider: params })),
          funds: []
        })
      }),

      deleteConfigs: (params: DeleteConfigsParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ delete_configs: params })),
          funds: []
        })
      }),

      setGatekeeper: (params: SetGatekeeperParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ set_gatekeeper: params })),
          funds: []
        })
      }),

      approveAll: (params: ApproveAllParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ approve_all: params })),
          funds: []
        })
      }),

      revokeAll: (params: RevokeAllParams): MsgExecuteContractEncodeObject => ({
        typeUrl: '/cosmwasm.wasm.v1.MsgExecuteContract',
        value: MsgExecuteContract.fromPartial({
          sender: this.sender,
          contract: this.contractAddress,
          msg: toUtf8(JSON.stringify({ revoke_all: params })),
          funds: []
        })
      })
    }
  }
}
