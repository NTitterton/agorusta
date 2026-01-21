import { type User, login as apiLogin, register as apiRegister, getMe, setToken, clearToken } from './api';

class AuthState {
	user = $state<User | null>(null);
	loading = $state(true);
	error = $state<string | null>(null);

	get isAuthenticated() {
		return this.user !== null;
	}

	async init() {
		this.loading = true;
		const token = typeof window !== 'undefined' ? localStorage.getItem('token') : null;

		if (token) {
			const result = await getMe();
			if (result.data) {
				this.user = result.data;
			} else {
				clearToken();
			}
		}

		this.loading = false;
	}

	async login(email: string, password: string): Promise<boolean> {
		this.error = null;
		const result = await apiLogin(email, password);

		if (result.error) {
			this.error = result.error;
			return false;
		}

		if (result.data) {
			setToken(result.data.token);
			this.user = result.data.user;
			return true;
		}

		return false;
	}

	async register(email: string, username: string, password: string): Promise<boolean> {
		this.error = null;
		const result = await apiRegister(email, username, password);

		if (result.error) {
			this.error = result.error;
			return false;
		}

		if (result.data) {
			setToken(result.data.token);
			this.user = result.data.user;
			return true;
		}

		return false;
	}

	logout() {
		clearToken();
		this.user = null;
	}
}

export const auth = new AuthState();
