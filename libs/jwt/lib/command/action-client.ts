import type { JwtStore } from '../index.js'
import type { Operation } from './operation.js'
import type { AnyFunc, HandleCancelFunc, HandleErrorFunc, HandleSuccessFunc, IStoreClient } from './types.js'
import { AbstractCommand } from './types.js'

export class StoreClient implements IStoreClient {
	private _store: JwtStore
	private _dispatchOperation: (op: Operation) => unknown
	private _isCancel: boolean
	private _successCbs: HandleSuccessFunc[]
	private _errorCbs: HandleErrorFunc[]
	private _cancelCbs: HandleCancelFunc[]
	private _commandFuncList: (() => AbstractCommand[])[]

	constructor(store: JwtStore, dispatchOperation: (op: Operation) => unknown, command?: AbstractCommand | AbstractCommand[] | (() => AbstractCommand[])) {
		this._store = store
		this._dispatchOperation = dispatchOperation
		this._successCbs = []
		this._errorCbs = []
		this._cancelCbs = []
		this._isCancel = false
		this._cancelApply = this._cancelApply.bind(this)
		this._commandFuncList = []
		command && this.next(command)
	}

	public next(command: AbstractCommand | AbstractCommand[] | (() => AbstractCommand[])) {
		let commandFunc = null
		if (command instanceof AbstractCommand) {
			commandFunc = () => [command]
		} else if (Array.isArray(command)) {
			commandFunc = () => command
		} else if (typeof command === 'function') {
			commandFunc = command
		}
		commandFunc && this._commandFuncList.push(commandFunc)
		return this
	}

	public success(successCb: AnyFunc) {
		this._successCbs.push(successCb)
		return this
	}

	public error(errorCb: HandleErrorFunc) {
		this._errorCbs.push(errorCb)
		return this
	}

	public cancel(cancelCb: AnyFunc) {
		this._cancelCbs.push(cancelCb)
		return this
	}

	public apply() {
		try {
			this._store.withTransact(() => {
				for (const commandFunc of this._commandFuncList) {
					const commands = commandFunc()
					for (const command of commands) {
						if (!command.validate(this._store, this._onError, this._cancelApply)) {
							this._cancelApply()
						}
						if (!command.apply(this._store, this._dispatchOperation, this._onError, this._cancelApply)) {
							this._cancelApply()
						}
						if (this._isCancel) {
							this._onCancel()
							return false
						}
					}
				}
				this._onSuccess()
				return true
			})
		} catch (e) {
			this._onCancel()
			throw e
		}
	}

	private _cancelApply() {
		this._isCancel = true
	}

	private _onSuccess() {
		let stop = false
		const stopPropagation = () => {
			stop = true
		}
		for (const successCb of this._successCbs) {
			successCb(stopPropagation)
			if (stop) {
				break
			}
		}
	}

	private _onError(errerCode: string, errerMsg: string) {
		let stop = false
		const stopPropagation = () => {
			stop = true
		}
		for (const errorCb of this._errorCbs) {
			errorCb(errerCode, errerMsg, this._cancelApply, stopPropagation)
			if (stop) {
				break
			}
		}
		return this._isCancel
	}

	private _onCancel() {
		let stop = false
		const stopPropagation = () => {
			stop = true
		}
		for (const cancelCb of this._cancelCbs) {
			cancelCb(stopPropagation)
			if (stop) {
				break
			}
		}
	}
}
