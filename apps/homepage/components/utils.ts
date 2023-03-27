export const nanoid = (size = 10) =>
	crypto.getRandomValues(new Uint8Array(size)).reduce((id, byte) => {
		byte &= 63
		if (byte < 36) {
			id += byte.toString(36)
		} else if (byte < 62) {
			id += (byte - 26).toString(36).toUpperCase()
		} else if (byte > 62) {
			id += '-'
		} else {
			id += '_'
		}
		return id
	}, '')
