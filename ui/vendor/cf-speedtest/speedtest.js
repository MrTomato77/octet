//#region src/config/defaultConfig.ts
const REL_API_URL = "https://speed.cloudflare.com";
const defaultConfig = {
	autoStart: true,
	downloadApiUrl: `${REL_API_URL}/__down`,
	uploadApiUrl: `${REL_API_URL}/__up`,
	logMeasurementApiUrl: null,
	logAimApiUrl: `${REL_API_URL}/__results`,
	turnServerUri: null,
	turnServerCredsApiUrl: null,
	turnServerUser: null,
	turnServerPass: null,
	rpkiInvalidHost: "invalid.rpki.cloudflare.com",
	includeCredentials: false,
	sessionId: void 0,
	authorizationToken: null,
	authorizationEnabled: void 0,
	allowInsecureAuthorizationToken: false,
	measurements: [
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "download",
			bytes: 1e5,
			count: 1,
			bypassMinDuration: true
		},
		{
			type: "latency",
			numPackets: 20
		},
		{
			type: "download",
			bytes: 1e5,
			count: 9
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "download",
			bytes: 1e6,
			count: 8
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "upload",
			bytes: 1e5,
			count: 8
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "upload",
			bytes: 1e6,
			count: 6
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "download",
			bytes: 1e7,
			count: 6
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "upload",
			bytes: 1e7,
			count: 4
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "download",
			bytes: 25e6,
			count: 4
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "upload",
			bytes: 25e6,
			count: 4
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "download",
			bytes: 1e8,
			count: 3
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "upload",
			bytes: 5e7,
			count: 3
		},
		{
			type: "latency",
			numPackets: 2
		},
		{
			type: "download",
			bytes: 25e7,
			count: 2
		}
	],
	measureDownloadLoadedLatency: true,
	measureUploadLoadedLatency: true,
	loadedLatencyThrottle: 400,
	bandwidthFinishRequestDuration: 1e3,
	estimatedServerTime: 0,
	bandwidthAbortRequestDuration: 0,
	latencyPercentile: .5,
	bandwidthPercentile: .9,
	bandwidthMinRequestDuration: 10,
	loadedRequestMinDuration: 250,
	loadedLatencyMaxPoints: 20
};
//#endregion
//#region src/utils/scaleThreshold.ts
const scaleThreshold = (domain, range) => {
	return (value) => {
		let i = 0;
		while (i < domain.length && value >= domain[i]) i++;
		return range[i];
	};
};
//#endregion
//#region src/config/internalConfig.ts
const internalConfig = {
	aimMeasurementScoring: {
		packetLoss: scaleThreshold([
			.01,
			.05,
			.25,
			.5
		], [
			10,
			5,
			0,
			-10,
			-20
		]),
		latency: scaleThreshold([
			10,
			20,
			50,
			100,
			500
		], [
			20,
			10,
			5,
			0,
			-10,
			-20
		]),
		loadedLatencyIncrease: scaleThreshold([
			10,
			20,
			50,
			100,
			500
		], [
			20,
			10,
			5,
			0,
			-10,
			-20
		]),
		jitter: scaleThreshold([
			10,
			20,
			100,
			500
		], [
			10,
			5,
			0,
			-10,
			-20
		]),
		download: scaleThreshold([
			1e6,
			1e7,
			5e7,
			1e8
		], [
			0,
			5,
			10,
			20,
			30
		]),
		upload: scaleThreshold([
			1e6,
			1e7,
			5e7,
			1e8
		], [
			0,
			5,
			10,
			20,
			30
		])
	},
	aimExperiencesDefs: {
		streaming: {
			input: [
				"latency",
				"packetLoss",
				"download",
				"loadedLatencyIncrease"
			],
			pointThresholds: [
				15,
				20,
				40,
				60
			]
		},
		gaming: {
			input: [
				"latency",
				"packetLoss",
				"loadedLatencyIncrease"
			],
			pointThresholds: [
				5,
				15,
				25,
				30
			]
		},
		rtc: {
			input: [
				"latency",
				"jitter",
				"packetLoss",
				"loadedLatencyIncrease"
			],
			pointThresholds: [
				5,
				15,
				25,
				40
			]
		}
	}
};
//#endregion
//#region src/utils/authorization.ts
const isSecureApiUrl = (apiUrl) => {
	try {
		return new URL(apiUrl).protocol === "https:";
	} catch {
		try {
			return new URL(apiUrl, window.location.origin).protocol === "https:";
		} catch {
			return false;
		}
	}
};
const warnedInsecure = new WeakSet();
const warnInsecureOnce = (authorization) => {
	if (warnedInsecure.has(authorization)) return;
	warnedInsecure.add(authorization);
	console.warn("Sending the authorization token to an endpoint that is not known to be secure, because allowInsecureAuthorizationToken is enabled. Do not enable this outside local development: the server treats a token seen over plaintext as compromised and revokes it.");
};
const withAuthorizationHeader = (init, authorization, apiUrl) => {
	if (!authorization?.enabled || !apiUrl) return init;
	const token = authorization.token?.trim();
	if (!token) return init;
	if (!isSecureApiUrl(apiUrl)) {
		if (!authorization.allowInsecure) return init;
		warnInsecureOnce(authorization);
	}
	const headers = new Headers(init.headers);
	headers.set("Authorization", `Bearer ${token}`);
	return {
		...init,
		headers: Object.fromEntries(headers)
	};
};
//#endregion
//#region src/engines/BandwidthEngine/BandwidthEngine.ts
const MAX_RETRIES = 3;
var HttpError = class extends Error {
	status;
	retryAfter;
	constructor(status, statusText, retryAfter) {
		super(statusText);
		this.status = status;
		this.retryAfter = retryAfter;
	}
};
const getRetryDelay = (retryAfter) => {
	if (!retryAfter) return 5e3;
	const seconds = Number(retryAfter);
	if (Number.isFinite(seconds) && seconds >= 0) return seconds * 1e3;
	const date = Date.parse(retryAfter);
	return Number.isNaN(date) ? 5e3 : Math.max(0, date - Date.now());
};
const SERVER_TIME_MIN_DURATION = .01;
const SERVER_TIME_DELTA_MAX = 15;
const SERVER_TIME_CALIBRATION_MAX = 150;
const SERVER_TIME_DELTA_WEIGHT = .75;
const cfGetServerTime = (r) => {
	const serverTiming = r.headers.get(`server-timing`);
	if (!serverTiming) return;
	const re = serverTiming.match(/(?:^|,\s*)cfReq(?:uest)?Dur(?:ation)?;\s*dur=([0-9.]+)/i);
	if (re && +re[1] > SERVER_TIME_MIN_DURATION) return +re[1];
	let sum = 0;
	for (const m of serverTiming.matchAll(/(?:^|,\s*)cfSpeed[a-zA-Z]*;\s*dur=([0-9.]+)/gi)) sum += +m[1];
	if (sum > SERVER_TIME_MIN_DURATION) return sum;
};
const getFinalResponseStart = (perf) => perf.finalResponseHeadersStart || perf.responseStart;
const getTtfb = (perf) => getFinalResponseStart(perf) - perf.requestStart;
const getPayloadDownload = (perf) => perf.responseEnd - getFinalResponseStart(perf);
const calcDownloadDuration = ({ ping, payloadDownloadTime }) => ping + payloadDownloadTime;
const calcUploadDuration = ({ ttfb }) => ttfb;
const calcDownloadSpeed = ({ duration, transferSize }, numBytes) => {
	const bits = 8 * (transferSize || +numBytes * 1.005);
	const secs = duration / 1e3;
	return !secs ? void 0 : bits / secs;
};
const calcUploadSpeed = ({ duration }, numBytes) => {
	const bits = 8 * numBytes * 1.005;
	const secs = duration / 1e3;
	return !secs ? void 0 : bits / secs;
};
const hasUsableSpeed = ({ bps }, numBytes) => +numBytes === 0 || typeof bps === "number" && Number.isFinite(bps) && bps > 0;
const genContent = (() => {
	const cache = new Map();
	return (numBytes) => {
		if (!cache.has(numBytes)) cache.set(numBytes, "0".repeat(numBytes));
		return cache.get(numBytes);
	};
})();
var BandwidthMeasurementEngine = class {
	constructor(measurements, { downloadApiUrl, uploadApiUrl, throttleMs = 0, estimatedServerTime = 0, serverTimeDelta = 0, authorization = null } = {}) {
		if (!measurements) throw new Error("Missing measurements argument");
		if (!downloadApiUrl) throw new Error("Missing downloadApiUrl argument");
		if (!uploadApiUrl) throw new Error("Missing uploadApiUrl argument");
		this.#measurements = measurements;
		this.#downloadApi = downloadApiUrl;
		this.#uploadApi = uploadApiUrl;
		this.#throttleMs = throttleMs;
		this.#estimatedServerTime = Math.max(0, estimatedServerTime);
		this.#serverTimeDelta = Math.max(0, serverTimeDelta);
		this.#authorization = authorization;
	}
	get results() {
		return this.#results;
	}
	get serverTimeDelta() {
		return this.#serverTimeDelta;
	}
	#qsParams = {};
	get qsParams() {
		return this.#qsParams;
	}
	set qsParams(v) {
		this.#qsParams = v;
	}
	#fetchOptions = {};
	get fetchOptions() {
		return this.#fetchOptions;
	}
	set fetchOptions(v) {
		this.#fetchOptions = v;
	}
	finishRequestDuration = 1e3;
	abortRequestDuration = 0;
	getServerTime = cfGetServerTime;
	#responseHook = () => {};
	set responseHook(f) {
		this.#responseHook = f;
	}
	#onRunningChange = () => {};
	set onRunningChange(f) {
		this.#onRunningChange = f;
	}
	#onNewMeasurementStarted = () => {};
	set onNewMeasurementStarted(f) {
		this.#onNewMeasurementStarted = f;
	}
	#onMeasurementResult = () => {};
	set onMeasurementResult(f) {
		this.#onMeasurementResult = f;
	}
	#onFinished = () => {};
	set onFinished(f) {
		this.#onFinished = f;
	}
	#onConnectionError = () => {};
	set onConnectionError(f) {
		this.#onConnectionError = f;
	}
	pause() {
		if (this.#retryTimeout !== void 0) {
			clearTimeout(this.#retryTimeout);
			this.#retryTimeout = void 0;
		}
		this.#cancelCurrentMeasurement(`pause()`);
		this.#setRunning(false);
	}
	play() {
		if (!this.#failed && !this.#running) {
			this.#setRunning(true);
			this.#nextMeasurement();
		}
	}
	#measurements;
	#downloadApi;
	#uploadApi;
	#running = false;
	#failed = false;
	#finished = {
		down: false,
		up: false
	};
	#results = {
		down: {},
		up: {}
	};
	#measIdx = 0;
	#counter = 0;
	#retries = 0;
	#retryTimeout;
	#minDuration = -Infinity;
	#throttleMs = 0;
	#estimatedServerTime = 0;
	#serverTimeDelta = 0;
	#authorization = null;
	#currentAbortController = void 0;
	#setRunning(running) {
		if (running !== this.#running) {
			this.#running = running;
			setTimeout(() => this.#onRunningChange(this.#running));
		}
		if (!running) this.#currentAbortController?.abort("setRunning(false)");
	}
	#saveMeasurementResults(measIdx, measTiming) {
		const { bytes, dir } = this.#measurements[measIdx];
		const results = this.#results;
		const bytesResult = results[dir].hasOwnProperty(bytes) ? results[dir][bytes] : {
			timings: [],
			numMeasurements: this.#measurements.filter(({ bytes: b, dir: d }) => bytes === b && dir === d).map((m) => m.count).reduce((agg, cnt) => agg + cnt, 0)
		};
		measTiming && bytesResult.timings.push(measTiming);
		bytesResult.timings = bytesResult.timings.slice(-bytesResult.numMeasurements);
		results[dir][bytes] = bytesResult;
		if (measTiming) setTimeout(() => {
			this.#onMeasurementResult({
				type: dir,
				bytes,
				...measTiming
			}, results);
		});
		else this.#onNewMeasurementStarted(this.#measurements[measIdx], results);
	}
	#nextMeasurement() {
		const measurements = this.#measurements;
		let meas = measurements[this.#measIdx];
		if (this.#counter >= meas.count) {
			const finished = this.#finished;
			if (this.#minDuration > this.finishRequestDuration && !meas.bypassMinDuration) {
				const dir = meas.dir;
				this.#finished[dir] = true;
				Object.values(this.#finished).every((finished) => finished) && this.#onFinished(this.#results);
			}
			this.#counter = 0;
			this.#minDuration = -Infinity;
			performance.clearResourceTimings();
			do
				this.#measIdx += 1;
			while (this.#measIdx < measurements.length && finished[measurements[this.#measIdx].dir]);
			if (this.#measIdx >= measurements.length) {
				this.#finished = {
					down: true,
					up: true
				};
				this.#setRunning(false);
				this.#onFinished(this.#results);
				return;
			}
			meas = measurements[this.#measIdx];
		}
		const measIdx = this.#measIdx;
		if (this.#counter === 0) this.#saveMeasurementResults(measIdx);
		const { bytes: numBytes, dir } = meas;
		const isDown = dir === "down";
		const apiUrl = isDown ? this.#downloadApi : this.#uploadApi;
		const qsParams = Object.assign({}, this.#qsParams);
		qsParams.bytes = `${numBytes}`;
		const urlObj = new URL(apiUrl, window.location.origin);
		Object.entries(qsParams).forEach(([k, v]) => urlObj.searchParams.set(k, v));
		const url = urlObj.href;
		const fetchOpt = withAuthorizationHeader(Object.assign({}, isDown ? {} : {
			method: "POST",
			body: genContent(numBytes)
		}, this.#fetchOptions), this.#authorization, url);
		if (this.#retries === 0 || !this.#currentAbortController || this.#currentAbortController.signal.aborted) {
			this.#currentAbortController?.abort("restarting engine");
			this.#currentAbortController = new AbortController();
			if (this.abortRequestDuration) {
				const abortTimeout = setTimeout(() => {
					const errorMessage = `${isDown ? "Download" : "Upload"} measurement of ${numBytes} bytes aborted. Measurement exceeded bandwidthAbortRequestDuration (${this.abortRequestDuration}ms)`;
					this.#retries = 0;
					this.pause();
					this.#failed = true;
					this.#onConnectionError(errorMessage);
				}, this.abortRequestDuration);
				this.#currentAbortController.signal.addEventListener("abort", () => clearTimeout(abortTimeout));
			}
		}
		let serverTime;
		fetch(url, {
			...fetchOpt,
			signal: this.#currentAbortController.signal
		}).then((r) => {
			if (r.ok) return r;
			throw new HttpError(r.status, r.statusText, r.headers.get("retry-after"));
		}).then((r) => {
			this.getServerTime && (serverTime = this.getServerTime(r));
			return r;
		}).then((r) => r.text().then((body) => {
			this.#responseHook({
				url,
				headers: r.headers,
				body
			});
			return body;
		})).then(() => {
			const perf = performance.getEntriesByName(url).slice(-1)[0];
			const timing = {
				transferSize: perf.transferSize,
				ttfb: getTtfb(perf),
				payloadDownloadTime: getPayloadDownload(perf),
				serverTime: serverTime || -1,
				measTime: new Date(),
				ping: 0,
				duration: 0,
				bps: void 0
			};
			let connectTime = 0;
			if (perf.secureConnectionStart > perf.connectStart) connectTime = perf.secureConnectionStart - perf.connectStart;
			else connectTime = perf.connectEnd - perf.connectStart;
			const protoMatch = perf.nextHopProtocol.match(/([0-9.]+)/);
			const httpVersion = protoMatch ? +protoMatch[1] : 0;
			if (serverTime && connectTime && httpVersion > 0 && httpVersion < 2) {
				const delta = Math.max(0, timing.ttfb - connectTime) - serverTime;
				if (delta > 0 && delta <= SERVER_TIME_DELTA_MAX && delta <= serverTime && serverTime <= SERVER_TIME_CALIBRATION_MAX) {
					this.#serverTimeDelta = this.#serverTimeDelta * .25 + delta * SERVER_TIME_DELTA_WEIGHT;
					console.log(`serverTimeDelta (estimated): ${this.#serverTimeDelta.toFixed(2)}ms`);
				} else if (delta > 0) console.log(`serverTimeDelta (skipped): ${delta.toFixed(2)}ms`);
			}
			const baseServerTime = serverTime || this.#estimatedServerTime;
			timing.ping = timing.ttfb - baseServerTime - this.#serverTimeDelta;
			if (timing.ping <= 1) timing.ping = Math.max(0, timing.ttfb - baseServerTime);
			timing.duration = (isDown ? calcDownloadDuration : calcUploadDuration)(timing);
			timing.bps = (isDown ? calcDownloadSpeed : calcUploadSpeed)(timing, numBytes);
			const delta = this.#serverTimeDelta;
			if (+numBytes === 0) console.log("latency", {
				phase: `during ${qsParams.during || "idle"}`,
				ttfb: timing.ttfb,
				serverTime: baseServerTime,
				...delta && { serverTimeDelta: delta },
				ping: timing.ping
			});
			else console.log(isDown ? "download" : "upload", {
				bytes: +numBytes,
				bps: timing.bps,
				ttfb: timing.ttfb,
				serverTime: baseServerTime,
				...delta && { serverTimeDelta: delta },
				ping: timing.ping
			});
			if (isDown && numBytes) {
				const reqSize = +numBytes;
				if (timing.transferSize && (timing.transferSize < reqSize || timing.transferSize / reqSize > 1.05)) console.warn(`Requested ${reqSize}B but received ${timing.transferSize}B (${Math.round(timing.transferSize / reqSize * 1e4) / 100}%).`);
			}
			if (hasUsableSpeed(timing, numBytes)) {
				this.#saveMeasurementResults(measIdx, timing);
				const requestDuration = timing.duration;
				this.#minDuration = this.#minDuration < 0 ? requestDuration : Math.min(this.#minDuration, requestDuration);
			} else console.warn(`Discarding ${dir} sample without a speed`, {
				bytes: +numBytes,
				...timing,
				perf: {
					startTime: perf.startTime,
					requestStart: perf.requestStart,
					responseStart: perf.responseStart,
					firstInterimResponseStart: perf.firstInterimResponseStart,
					finalResponseHeadersStart: perf.finalResponseHeadersStart,
					responseEnd: perf.responseEnd,
					nextHopProtocol: perf.nextHopProtocol,
					entries: performance.getEntriesByName(url).length
				}
			});
			this.#counter += 1;
			this.#retries = 0;
			if (this.#throttleMs) {
				const throttleTimeout = setTimeout(() => this.#nextMeasurement(), this.#throttleMs);
				this.#currentAbortController.signal.addEventListener("abort", () => clearTimeout(throttleTimeout));
			} else this.#nextMeasurement();
		}).catch((error) => {
			if (this.#currentAbortController.signal.aborted) return;
			console.warn(`Error fetching ${url}: ${error}`);
			if (error instanceof HttpError && error.status === 429 && this.#retries++ < MAX_RETRIES) {
				this.#retryTimeout = setTimeout(() => {
					this.#retryTimeout = void 0;
					if (this.#running && !this.#failed) this.#nextMeasurement();
				}, getRetryDelay(error.retryAfter));
				return;
			}
			const status = error instanceof HttpError ? error.status : void 0;
			const message = status === 429 ? `Connection failed to ${url}. Gave up after ${MAX_RETRIES} retries.` : status === void 0 ? `Connection failed to ${url}.` : `Request failed with ${status}: ${url}`;
			this.#retries = 0;
			this.pause();
			this.#failed = true;
			this.#onConnectionError(message, status);
		});
	}
	#cancelCurrentMeasurement(reason) {
		this.#currentAbortController?.abort(reason || `aborted with no reason provided`);
	}
};
//#endregion
//#region src/engines/BandwidthEngine/ParallelLatency.ts
var BandwidthWithParallelLatencyEngine = class extends BandwidthMeasurementEngine {
	constructor(measurements, { measureParallelLatency = false, parallelLatencyThrottleMs = 100, downloadApiUrl, uploadApiUrl, estimatedServerTime = 0, serverTimeDelta = 0, authorization = null, ...ptProps } = {}) {
		super(measurements, {
			downloadApiUrl,
			uploadApiUrl,
			estimatedServerTime,
			serverTimeDelta,
			authorization,
			...ptProps
		});
		if (measureParallelLatency) {
			this.#latencyEngine = new BandwidthMeasurementEngine([{
				dir: "down",
				bytes: 0,
				count: Infinity,
				bypassMinDuration: true
			}], {
				downloadApiUrl,
				uploadApiUrl,
				estimatedServerTime,
				serverTimeDelta,
				authorization,
				throttleMs: parallelLatencyThrottleMs
			});
			this.#latencyEngine.qsParams = { during: `${measurements[0].dir}load` };
			super.onRunningChange = this.#setLatencyRunning;
			super.onConnectionError = () => this.#latencyEngine.pause();
		}
	}
	get latencyResults() {
		return this.#latencyEngine && this.#latencyEngine.results.down[0].timings;
	}
	set onParallelLatencyResult(f) {
		this.#latencyEngine && (this.#latencyEngine.onMeasurementResult = (res) => f(res));
	}
	get fetchOptions() {
		return super.fetchOptions;
	}
	set fetchOptions(fetchOptions) {
		super.fetchOptions = fetchOptions;
		this.#latencyEngine && (this.#latencyEngine.fetchOptions = fetchOptions);
	}
	set onRunningChange(onRunningChange) {
		super.onRunningChange = (running) => {
			this.#setLatencyRunning(running);
			onRunningChange(running);
		};
	}
	set onConnectionError(onConnectionError) {
		super.onConnectionError = (...args) => {
			this.#latencyEngine && this.#latencyEngine.pause();
			onConnectionError(...args);
		};
		if (this.#latencyEngine) this.#latencyEngine.onConnectionError = (...args) => {
			this.pause();
			onConnectionError(...args);
		};
	}
	#latencyEngine;
	#latencyTimeout;
	#setLatencyRunning = (running) => {
		if (this.#latencyEngine) {
			if (!running) {
				clearTimeout(this.#latencyTimeout);
				this.#latencyEngine.pause();
			} else this.#latencyTimeout = setTimeout(() => this.#latencyEngine.play(), 20);
		}
	};
};
//#endregion
//#region src/engines/BandwidthEngine/LoggingBandwidthEngine.ts
var LoggingBandwidthEngine = class extends BandwidthWithParallelLatencyEngine {
	constructor(measurements, { measurementId, logApiUrl, sessionId, authorization, ...ptProps } = {}) {
		super(measurements, {
			...ptProps,
			authorization
		});
		this.#measurementId = measurementId;
		this.#logApiUrl = logApiUrl;
		this.#sessionId = sessionId;
		this.#authorization = authorization ?? null;
		super.qsParams = logApiUrl ? { measId: this.#measurementId } : {};
		super.responseHook = (r) => this.#loggingResponseHook(r);
		super.onMeasurementResult = (meas) => this.#logMeasurement(meas);
	}
	set qsParams(qsParams) {
		super.qsParams = this.#logApiUrl ? {
			measId: this.#measurementId,
			...qsParams
		} : qsParams;
	}
	set responseHook(responseHook) {
		super.responseHook = (r) => {
			responseHook(r);
			this.#loggingResponseHook(r);
		};
	}
	set onMeasurementResult(onMeasurementResult) {
		super.onMeasurementResult = (meas, ...restArgs) => {
			onMeasurementResult(meas, ...restArgs);
			this.#logMeasurement(meas);
		};
	}
	#measurementId;
	#token;
	#requestTime;
	#logApiUrl;
	#sessionId;
	#authorization;
	#loggingResponseHook(r) {
		if (!this.#logApiUrl) return;
		this.#requestTime = +r.headers.get(`cf-meta-request-time`);
		this.#token = r.body.slice(-300).split("___").pop();
	}
	#logMeasurement(measData) {
		if (!this.#logApiUrl) return;
		const logData = {
			type: measData.type,
			bytes: measData.bytes,
			ping: Math.round(measData.ping),
			ttfb: Math.round(measData.ttfb),
			payloadDownloadTime: Math.round(measData.payloadDownloadTime),
			duration: Math.round(measData.duration),
			transferSize: Math.round(measData.transferSize),
			serverTime: Math.round(measData.serverTime),
			token: this.#token,
			requestTime: this.#requestTime,
			measId: this.#measurementId,
			sessionId: this.#sessionId
		};
		this.#token = null;
		this.#requestTime = null;
		fetch(this.#logApiUrl, withAuthorizationHeader({
			method: "POST",
			body: JSON.stringify(logData),
			...this.fetchOptions
		}, this.#authorization, this.#logApiUrl)).catch(() => {});
	}
};
//#endregion
//#region src/engines/LoadNetworkEngine/index.ts
var PromiseEngine = class {
	constructor(promiseFn) {
		if (!promiseFn) throw new Error(`Missing operation to perform`);
		this.#promiseFn = promiseFn;
		this.play();
	}
	pause() {
		this.#cancelCurrent();
		this.#setRunning(false);
	}
	stop() {
		this.pause();
	}
	play() {
		if (!this.#running) {
			this.#setRunning(true);
			this.#next();
		}
	}
	#running = false;
	#currentPromise = void 0;
	#promiseFn;
	#setRunning(running) {
		if (running !== this.#running) this.#running = running;
	}
	#next() {
		const curPromise = this.#currentPromise = this.#promiseFn().then(() => {
			!curPromise._cancel && this.#next();
		});
	}
	#cancelCurrent() {
		const curPromise = this.#currentPromise;
		curPromise && (curPromise._cancel = true);
	}
};
var LoadNetworkEngine = class {
	constructor({ download, upload, authorization = null } = {}) {
		if (!download && !upload) throw new Error("Missing at least one of download/upload config");
		[[download, "download"], [upload, "upload"]].filter((entry) => entry[0] !== null && entry[0] !== void 0).forEach(([cfg, type]) => {
			const { apiUrl, chunkSize } = cfg;
			if (!apiUrl) throw new Error(`Missing ${type} apiUrl argument`);
			if (!chunkSize) throw new Error(`Missing ${type} chunkSize argument`);
		});
		const getLoadEngine = ({ apiUrl, qsParams = {}, fetchOptions = {} }) => new PromiseEngine(() => {
			const fetchQsParams = Object.assign({}, qsParams, this.qsParams);
			const urlObj = new URL(apiUrl, window.location.origin);
			Object.entries(fetchQsParams).forEach(([k, v]) => urlObj.searchParams.set(k, v));
			const url = urlObj.href;
			const fetchOpt = Object.assign({}, fetchOptions, this.fetchOptions);
			return fetch(url, withAuthorizationHeader(fetchOpt, authorization, url)).then((r) => {
				if (r.ok) return r;
				throw Error(r.statusText);
			}).then((r) => r.text());
		});
		download && this.#engines.push(getLoadEngine({
			apiUrl: download.apiUrl,
			qsParams: { bytes: `${download.chunkSize}` }
		}));
		upload && this.#engines.push(getLoadEngine({
			apiUrl: upload.apiUrl,
			fetchOptions: {
				method: "POST",
				body: "0".repeat(upload.chunkSize)
			}
		}));
	}
	qsParams = {};
	fetchOptions = {};
	pause() {
		this.#engines.forEach((engine) => engine.pause());
	}
	stop() {
		this.pause();
	}
	play() {
		this.#engines.forEach((engine) => engine.play());
	}
	#engines = [];
};
//#endregion
//#region src/engines/PacketLossEngine/SelfWebRtcDataConnection.ts
var SelfWebRtcDataConnection = class {
	constructor({ iceServers = [], acceptIceCandidate = (candidate) => {
		let protocol = candidate.protocol || "";
		if (!protocol && candidate.candidate) {
			const sdpAttrs = candidate.candidate.split(" ");
			sdpAttrs.length >= 3 && (protocol = sdpAttrs[2]);
		}
		return protocol.toLowerCase() === "udp";
	}, dataChannelCfg = {
		ordered: false,
		maxRetransmits: 0
	}, ...rtcPeerConnectionCfg } = {}) {
		const sender = new RTCPeerConnection({
			iceServers,
			...rtcPeerConnectionCfg
		});
		const receiver = new RTCPeerConnection({
			iceServers,
			...rtcPeerConnectionCfg
		});
		const senderDc = sender.createDataChannel("channel", dataChannelCfg);
		senderDc.onopen = () => {
			this.#established = true;
			this.onOpen();
		};
		senderDc.onclose = () => this.close();
		receiver.ondatachannel = (e) => {
			const dc = e.channel;
			dc.onclose = () => this.close();
			dc.onmessage = (msg) => this.onMessageReceived(msg.data);
			this.#receiverDc = dc;
		};
		sender.onicecandidate = (e) => {
			e.candidate && acceptIceCandidate(e.candidate) && receiver.addIceCandidate(e.candidate);
		};
		receiver.onicecandidate = (e) => {
			e.candidate && acceptIceCandidate(e.candidate) && sender.addIceCandidate(e.candidate);
		};
		sender.createOffer().then((offer) => sender.setLocalDescription(offer)).then(() => receiver.setRemoteDescription(sender.localDescription)).then(() => receiver.createAnswer()).then((answer) => receiver.setLocalDescription(answer)).then(() => sender.setRemoteDescription(receiver.localDescription));
		this.#sender = sender;
		this.#receiver = receiver;
		this.#senderDc = senderDc;
	}
	onOpen = () => {};
	onClose = () => {};
	onMessageReceived = () => {};
	send(msg) {
		this.#senderDc.send(String(msg));
	}
	close() {
		this.#sender && this.#sender.close();
		this.#receiver && this.#receiver.close();
		this.#senderDc && this.#senderDc.close();
		this.#receiverDc && this.#receiverDc.close();
		this.#established && this.onClose();
		this.#established = false;
		return this;
	}
	#established = false;
	#sender;
	#receiver;
	#senderDc;
	#receiverDc;
};
//#endregion
//#region src/engines/PacketLossEngine/PacketLossEngine.ts
var PacketLossEngine = class {
	constructor({ turnServerUri, turnServerCredsApi, turnServerCredsApiParser = ({ username, credential, server }) => ({
		turnServerUser: username,
		turnServerPass: credential,
		turnServerUri: server
	}), turnServerCredsApiIncludeCredentials = false, turnServerUser, turnServerPass, authorization = null, numMsgs = 100, batchSize = 10, batchWaitTime = 10, responsesWaitTime = 5e3, connectionTimeout = 5e3 } = {}) {
		if (!turnServerUri && !turnServerCredsApi) throw new Error("No TURN server configured: set turnServerCredsApiUrl or turnServerUri");
		if ((!turnServerUser || !turnServerPass) && !turnServerCredsApi) throw new Error("No TURN credentials configured: set turnServerCredsApiUrl, or turnServerUser and turnServerPass");
		this.#numMsgs = numMsgs;
		(!turnServerUser || !turnServerPass ? fetch(turnServerCredsApi, withAuthorizationHeader(turnServerCredsApiIncludeCredentials ? { credentials: "include" } : {}, authorization, turnServerCredsApi)).then((r) => r.json()).then((d) => {
			if (d.error) throw d.error;
			return d;
		}).then(turnServerCredsApiParser) : Promise.resolve({
			turnServerUser,
			turnServerPass
		})).catch((e) => this.#onCredentialsFailure(e)).then((creds) => {
			if (!creds) return;
			const { turnServerUser: credsUser, turnServerPass: credsPass, turnServerUri: credsApiTurnServerUri } = creds;
			const uri = credsApiTurnServerUri || turnServerUri;
			if (!uri) {
				this.#onConnectionError("No TURN server URI: set turnServerUri or return server from the credentials API");
				return;
			}
			const c = new SelfWebRtcDataConnection({
				iceServers: [{
					urls: `turn:${uri}?transport=udp`,
					username: credsUser,
					credential: credsPass
				}],
				iceTransportPolicy: "relay"
			});
			let connectionSuccess = false;
			setTimeout(() => {
				if (!connectionSuccess) {
					c.close();
					this.#onConnectionError("ICE connection timeout!");
				}
			}, connectionTimeout);
			const msgTracker = this.#msgTracker;
			c.onOpen = () => {
				connectionSuccess = true;
				const self = this;
				(function sendNum(n) {
					if (n <= numMsgs) {
						let i = n;
						while (i <= Math.min(numMsgs, n + batchSize - 1)) {
							msgTracker[i] = false;
							c.send(i);
							self.onMsgSent(i);
							i++;
						}
						setTimeout(() => sendNum(i), batchWaitTime);
					} else {
						self.onAllMsgsSent(Object.keys(msgTracker).length);
						const finishFn = () => {
							c.close();
							self.#onFinished(self.results);
						};
						let finishTimeout = setTimeout(finishFn, responsesWaitTime);
						let missingMsgs = Object.values(self.#msgTracker).filter((recv) => !recv).length;
						c.onMessageReceived = (msg) => {
							clearTimeout(finishTimeout);
							msgTracker[msg] = true;
							self.onMsgReceived(msg);
							missingMsgs--;
							if (missingMsgs <= 0 && Object.values(self.#msgTracker).every((recv) => recv)) finishFn();
							else finishTimeout = setTimeout(finishFn, responsesWaitTime);
						};
					}
				})(1);
			};
			c.onMessageReceived = (msg) => {
				msgTracker[msg] = true;
				this.onMsgReceived(msg);
			};
		}).catch((e) => this.#onConnectionError(e.toString()));
	}
	#onCredentialsFailure = () => {};
	set onCredentialsFailure(f) {
		this.#onCredentialsFailure = f;
	}
	#onConnectionError = () => {};
	set onConnectionError(f) {
		this.#onConnectionError = f;
	}
	#onFinished = () => {};
	set onFinished(f) {
		this.#onFinished = f;
	}
	onMsgSent = () => {};
	onAllMsgsSent = () => {};
	onMsgReceived = () => {};
	get results() {
		const totalMessages = this.#numMsgs;
		const numMessagesSent = Object.keys(this.#msgTracker).length;
		const lostMessages = Object.entries(this.#msgTracker).filter(([, recv]) => !recv).map(([n]) => +n);
		return {
			totalMessages,
			numMessagesSent,
			packetLoss: lostMessages.length / numMessagesSent,
			lostMessages
		};
	}
	#msgTracker = {};
	#numMsgs;
};
//#endregion
//#region src/engines/PacketLossEngine/UnderLoad.ts
var PacketLossUnderLoadEngine = class extends PacketLossEngine {
	constructor({ downloadChunkSize, uploadChunkSize, downloadApiUrl, uploadApiUrl, authorization, ...ptProps } = {}) {
		super({
			...ptProps,
			authorization
		});
		if (downloadChunkSize || uploadChunkSize) {
			this.#loadEngine = new LoadNetworkEngine({
				download: downloadChunkSize ? {
					apiUrl: downloadApiUrl,
					chunkSize: downloadChunkSize
				} : null,
				upload: uploadChunkSize ? {
					apiUrl: uploadApiUrl,
					chunkSize: uploadChunkSize
				} : null,
				authorization
			});
			super.onCredentialsFailure = super.onConnectionError = super.onFinished = () => this.#loadEngine.stop();
		}
	}
	set qsParams(qsParams) {
		this.#loadEngine && (this.#loadEngine.qsParams = qsParams);
	}
	set fetchOptions(fetchOptions) {
		this.#loadEngine && (this.#loadEngine.fetchOptions = fetchOptions);
	}
	set onCredentialsFailure(onCredentialsFailure) {
		super.onCredentialsFailure = (...args) => {
			onCredentialsFailure(...args);
			this.#loadEngine && this.#loadEngine.stop();
		};
	}
	set onConnectionError(onConnectionError) {
		super.onConnectionError = (...args) => {
			onConnectionError(...args);
			this.#loadEngine && this.#loadEngine.stop();
		};
	}
	set onFinished(onFinished) {
		super.onFinished = (...args) => {
			onFinished(...args);
			this.#loadEngine && this.#loadEngine.stop();
		};
	}
	#loadEngine;
};
//#endregion
//#region src/engines/ReachabilityEngine/index.ts
var ReachabilityEngine = class {
	constructor(targetUrl, { timeout = -1, fetchOptions = {} } = {}) {
		let finished = false;
		const finish = ({ reachable, ...rest }) => {
			if (finished) return;
			finished = true;
			this.onFinished({
				targetUrl,
				reachable,
				...rest
			});
		};
		fetch(targetUrl, fetchOptions).then((response) => {
			finish({
				reachable: true,
				response
			});
		}).catch((error) => {
			finish({
				reachable: false,
				error
			});
		});
		timeout > 0 && setTimeout(() => finish({
			reachable: false,
			error: "Request timeout"
		}), timeout);
	}
	onFinished = () => {};
};
//#endregion
//#region src/utils/numbers.ts
const sum = (vals) => vals.reduce((agg, val) => agg + val, 0);
const percentile = (vals, perc = .5) => {
	if (!vals.length) return 0;
	const sortedVals = vals.slice().sort((a, b) => a - b);
	const idx = (vals.length - 1) * perc;
	const rem = idx % 1;
	if (rem === 0) return sortedVals[Math.round(idx)];
	const edges = [Math.floor, Math.ceil].map((rndFn) => sortedVals[rndFn(idx)]);
	return edges[0] + (edges[1] - edges[0]) * rem;
};
//#endregion
//#region src/Results/MeasurementCalculations.ts
var MeasurementCalculations = class {
	constructor(config) {
		this.#config = config;
	}
	getLatencyPoints = (latencyResults) => latencyResults.timings.map((d) => d.ping);
	getLatency = (latencyResults) => percentile(this.getLatencyPoints(latencyResults), this.#config.latencyPercentile);
	getJitter(latencyResults) {
		const pings = this.getLatencyPoints(latencyResults);
		return pings.length < 2 ? null : pings.reduce(({ sumDeltas = 0, prevLatency }, latency) => ({
			sumDeltas: sumDeltas + (prevLatency !== void 0 ? Math.abs(prevLatency - latency) : 0),
			prevLatency: latency
		}), {}).sumDeltas / (pings.length - 1);
	}
	getBandwidthPoints = (bandwidthResults) => Object.entries(bandwidthResults).map(([bytes, { timings }]) => timings.map(({ bps, duration, ping, measTime, serverTime, transferSize }) => ({
		bytes: +bytes,
		bps,
		duration,
		ping,
		measTime,
		serverTime,
		transferSize
	}))).flat();
	getBandwidth = (bandwidthResults) => percentile(this.getBandwidthPoints(bandwidthResults).filter((d) => d.duration >= this.#config.bandwidthMinRequestDuration).map((d) => d.bps).filter((bps) => bps), this.#config.bandwidthPercentile);
	getLoadedLatency = (loadedResults) => this.getLatency({ timings: this.#extractLoadedLatencies(loadedResults) });
	getLoadedJitter = (loadedResults) => this.getJitter({ timings: this.#extractLoadedLatencies(loadedResults) });
	getLoadedLatencyPoints = (loadedResults) => this.getLatencyPoints({ timings: this.#extractLoadedLatencies(loadedResults) });
	getPacketLoss = (plResults) => plResults.packetLoss;
	getPacketLossDetails = (plResults) => plResults;
	getReachability = (reachabilityResults) => !!reachabilityResults.reachable;
	getReachabilityDetails = (d) => ({
		host: d.host,
		reachable: d.reachable
	});
	#config;
	#extractLoadedLatencies = (loadedResults) => Object.values(loadedResults).filter((d) => d.timings.length && Math.min(...d.timings.map((d) => d.duration)) >= this.#config.loadedRequestMinDuration).map((d) => d.sideLatency || []).flat().slice(-this.#config.loadedLatencyMaxPoints);
};
//#endregion
//#region src/Results/ScoresCalculations.ts
const classificationNames = [
	"bad",
	"poor",
	"average",
	"good",
	"great"
];
const customResultTypes = { loadedLatencyIncrease: (measurements) => measurements.latency && (measurements.downLoadedLatency || measurements.upLoadedLatency) ? Math.max(measurements.downLoadedLatency, measurements.upLoadedLatency) - measurements.latency : void 0 };
const defaultPoints = { packetLoss: 0 };
var ScoresCalculations = class {
	constructor(config) {
		this.#config = config;
	}
	getScores(measurements, omit = []) {
		const scores = Object.assign({}, ...Object.entries(this.#config.aimMeasurementScoring).map(([type, fn]) => {
			const val = customResultTypes.hasOwnProperty(type) ? customResultTypes[type](measurements) : measurements[type];
			return val === void 0 ? defaultPoints.hasOwnProperty(type) ? { [type]: defaultPoints[type] } : {} : { [type]: +fn(val) };
		}));
		return Object.assign({}, ...Object.entries(this.#config.aimExperiencesDefs).map(([k, { input, pointThresholds }]) => [k, {
			input: input.filter((m) => !omit.includes(m)),
			pointThresholds
		}]).filter(([, { input }]) => input.every((k) => scores.hasOwnProperty(k))).map(([k, { input, pointThresholds }]) => {
			const sumPoints = Math.max(0, sum(input.map((k) => scores[k])));
			const classificationIdx = scaleThreshold(pointThresholds, [
				0,
				1,
				2,
				3,
				4
			])(sumPoints);
			const classificationName = classificationNames[classificationIdx];
			return { [k]: {
				points: sumPoints,
				classificationIdx,
				classificationName
			} };
		}));
	}
	#config;
};
//#endregion
//#region src/Results/index.ts
var Results = class {
	constructor(config) {
		this.#config = config;
		this.clear();
		this.#measCalc = new MeasurementCalculations(this.#config);
		this.#scoresCalc = new ScoresCalculations(this.#config);
	}
	raw;
	get isFinished() {
		return Object.values(this.raw).filter((d) => d !== null && typeof d === "object").every((d) => d.finished);
	}
	clear() {
		this.raw = Object.assign({ totalDurationMs: void 0 }, ...[...new Set(this.#config.measurements.map((m) => m.type))].map((m) => ({ [m]: {
			started: false,
			finished: false,
			results: {}
		} })));
	}
	getUnloadedLatency = () => this.#calcGetter("getLatency", "latency");
	getUnloadedJitter = () => this.#calcGetter("getJitter", "latency");
	getUnloadedLatencyPoints = () => this.#calcGetter("getLatencyPoints", "latency", []);
	getDownLoadedLatency = () => this.#calcGetter("getLoadedLatency", "download");
	getDownLoadedJitter = () => this.#calcGetter("getLoadedJitter", "download");
	getDownLoadedLatencyPoints = () => this.#calcGetter("getLoadedLatencyPoints", "download", []);
	getUpLoadedLatency = () => this.#calcGetter("getLoadedLatency", "upload");
	getUpLoadedJitter = () => this.#calcGetter("getLoadedJitter", "upload");
	getUpLoadedLatencyPoints = () => this.#calcGetter("getLoadedLatencyPoints", "upload", []);
	getDownloadBandwidth = () => this.#calcGetter("getBandwidth", "download");
	getDownloadBandwidthPoints = () => this.#calcGetter("getBandwidthPoints", "download", []);
	getUploadBandwidth = () => this.#calcGetter("getBandwidth", "upload");
	getUploadBandwidthPoints = () => this.#calcGetter("getBandwidthPoints", "upload", []);
	getPacketLoss = () => this.#calcGetter("getPacketLoss", "packetLoss");
	getPacketLossDetails = () => this.#calcGetter("getPacketLossDetails", "packetLoss", void 0, true);
	getTotalDurationMs = () => this.raw.totalDurationMs;
	getSummary() {
		const items = {
			download: this.getDownloadBandwidth,
			upload: this.getUploadBandwidth,
			latency: this.getUnloadedLatency,
			jitter: this.getUnloadedJitter,
			downLoadedLatency: this.getDownLoadedLatency,
			downLoadedJitter: this.getDownLoadedJitter,
			upLoadedLatency: this.getUpLoadedLatency,
			upLoadedJitter: this.getUpLoadedJitter,
			packetLoss: this.getPacketLoss,
			v4Reachability: this.#getV4Reachability,
			v6Reachability: this.#getV6Reachability,
			totalDurationMs: this.getTotalDurationMs
		};
		return Object.assign({}, ...Object.entries(items).map(([key, fn]) => {
			const val = fn();
			return val === void 0 ? {} : { [key]: val };
		}));
	}
	getScores = () => this.#scoresCalc.getScores(this.getSummary(), this.raw.packetLoss ? [] : ["packetLoss"]);
	#config;
	#measCalc;
	#scoresCalc;
	#calcGetter = (calcFn, resKey, defaultVal = void 0, surfaceError = false) => {
		const entry = this.raw[resKey];
		if (!entry || typeof entry !== "object" || !entry.started) return defaultVal;
		const measEntry = entry;
		if (surfaceError && measEntry.error) return { error: measEntry.error };
		return this.#measCalc[calcFn](measEntry.results);
	};
	#getV4Reachability = () => this.#calcGetter("getReachability", "v4Reachability");
	#getV4ReachabilityDetails = () => this.#calcGetter("getReachabilityDetails", "v4Reachability");
	#getV6Reachability = () => this.#calcGetter("getReachability", "v6Reachability");
	#getV6ReachabilityDetails = () => this.#calcGetter("getReachabilityDetails", "v6Reachability");
};
//#endregion
//#region src/logging/logFinalResults.ts
const round = (num, decimals = 0) => !num ? num : Math.round(num * 10 ** decimals) / 10 ** decimals;
const latencyPointsParser = (durations) => durations.map((d) => round(d, 2));
const bpsPointsParser = (pnts) => pnts.map((d) => ({
	bytes: +d.bytes,
	bps: round(d.bps)
}));
const packetLossParser = (d) => {
	const details = d;
	return details.error ? void 0 : {
		numMessages: details.numMessagesSent,
		lossRatio: round(details.packetLoss, 4)
	};
};
const resultsParsers = {
	latencyMs: ["getUnloadedLatencyPoints", latencyPointsParser],
	download: ["getDownloadBandwidthPoints", bpsPointsParser],
	upload: ["getUploadBandwidthPoints", bpsPointsParser],
	downLoadedLatencyMs: ["getDownLoadedLatencyPoints", latencyPointsParser],
	upLoadedLatencyMs: ["getUpLoadedLatencyPoints", latencyPointsParser],
	packetLoss: ["getPacketLossDetails", packetLossParser],
	totalDurationMs: ["getTotalDurationMs"]
};
const scoreParser = (d) => ({
	points: d.points,
	classification: d.classificationName
});
const logAimResults = async (results, { apiUrl, includeCredentials, sessionId, authorization }) => {
	const logData = { sessionId };
	Object.entries(resultsParsers).forEach(([logK, [fn, parser]]) => {
		const resolvedParser = parser ?? ((d) => d);
		const val = results[fn]();
		if (val) logData[logK] = resolvedParser(val);
	});
	const scores = results.getScores();
	if (scores) logData.scores = Object.assign({}, ...Object.entries(scores).map(([k, score]) => ({ [k]: scoreParser(score) })));
	console.log("results", logData);
	try {
		const response = await fetch(apiUrl, withAuthorizationHeader({
			method: "POST",
			body: JSON.stringify(logData),
			credentials: includeCredentials ? "include" : "same-origin"
		}, authorization, apiUrl));
		if (!response.ok) return { requestId: void 0 };
		return await response.json();
	} catch {
		return { requestId: void 0 };
	}
};
//#endregion
//#region src/index.ts
const DEFAULT_OPTIMAL_DOWNLOAD_SIZE = 1e6;
const DEFAULT_OPTIMAL_UPLOAD_SIZE = 1e6;
const OPTIMAL_SIZE_RATIO = .5;
const pausableTypes = [
	"latency",
	"latencyUnderLoad",
	"download",
	"upload"
];
const genMeasId = () => `${Math.round(Math.random() * 0x2386f26fc10000)}`;
var MeasurementEngine = class {
	constructor(userConfig = {}) {
		this.#config = Object.assign({}, defaultConfig, userConfig, internalConfig);
		this.#authorization = {
			token: this.#config.authorizationToken,
			enabled: this.#config.authorizationEnabled,
			allowInsecure: this.#config.allowInsecureAuthorizationToken
		};
		this.#results = new Results(this.#config);
		this.#config.autoStart && this.play();
	}
	get results() {
		return this.#results;
	}
	get config() {
		return this.#config;
	}
	get authorization() {
		return this.#authorization;
	}
	get isRunning() {
		return this.#running;
	}
	get isFinished() {
		return this.#finished;
	}
	onRunningChange = () => {};
	onResultsChange = () => {};
	onPhaseChange = () => {};
	#onFinish = () => {};
	set onFinish(f) {
		this.#onFinish = f;
	}
	#onError = () => {};
	set onError(f) {
		this.#onError = f;
	}
	pause() {
		const curType = this.#curType();
		curType && pausableTypes.includes(curType) && this.#curEngine?.pause?.();
		this.#setRunning(false);
	}
	play() {
		if (!this.#failed && !this.#running) {
			performance.clearResourceTimings();
			performance.setResourceTimingBufferSize(1e4);
			this.#setRunning(true);
			this.#next();
		}
	}
	restart() {
		this.#clear();
		this.play();
	}
	#config;
	#authorization;
	#results;
	#measurementId = genMeasId();
	#serverTimeDelta = 0;
	#curMsmIdx = -1;
	#curEngine;
	#optimalDownloadChunkSize = DEFAULT_OPTIMAL_DOWNLOAD_SIZE;
	#optimalUploadChunkSize = DEFAULT_OPTIMAL_UPLOAD_SIZE;
	#startTime;
	#accumulatedRuntimeMs = 0;
	#running = false;
	#finished = false;
	#failed = false;
	#setRunning(running) {
		if (running !== this.#running) {
			this.#running = running;
			this.onRunningChange(this.#running);
		}
		if (running) this.#startTime = performance.now();
		else if (typeof this.#startTime !== "undefined") {
			this.#accumulatedRuntimeMs += performance.now() - this.#startTime;
			this.#startTime = void 0;
		}
	}
	#setFinished(finished) {
		if (finished !== this.#finished) {
			this.#finished = finished;
			if (finished) {
				this.#results.raw.totalDurationMs = this.#accumulatedRuntimeMs;
				setTimeout(() => this.#onFinish(this.results));
			}
		}
	}
	#curType() {
		return this.#curMsmIdx < 0 || this.#curMsmIdx >= this.#config.measurements.length ? null : this.#config.measurements[this.#curMsmIdx].type;
	}
	#curTypeResults() {
		const type = this.#curType();
		if (!type) return void 0;
		return this.#results.raw[type] || void 0;
	}
	#clear() {
		this.#destroyCurEngine();
		this.#measurementId = genMeasId();
		this.#curMsmIdx = -1;
		this.#curEngine = void 0;
		this.#setRunning(false);
		this.#setFinished(false);
		this.#failed = false;
		this.#results.clear();
		this.#accumulatedRuntimeMs = 0;
	}
	#destroyCurEngine() {
		const engine = this.#curEngine;
		if (!engine) return;
		engine.onFinished = engine.onConnectionError = engine.onMsgReceived = engine.onCredentialsFailure = engine.onMeasurementResult = () => {};
		const curType = this.#curType();
		curType && pausableTypes.includes(curType) && engine.pause?.();
	}
	#next() {
		const resumeType = this.#curType();
		const resumeResults = this.#curTypeResults();
		if (resumeType && pausableTypes.includes(resumeType) && resumeResults && resumeResults.started && !resumeResults.finished && !resumeResults.finishedCurrentRound && !resumeResults.error) {
			this.#curEngine?.play?.();
			return;
		}
		this.#curMsmIdx++;
		if (this.#curMsmIdx >= this.#config.measurements.length) {
			this.#setRunning(false);
			this.#setFinished(true);
			return;
		}
		const { type, ...msmConfig } = this.#config.measurements[this.#curMsmIdx];
		const msmResults = this.#curTypeResults();
		this.onPhaseChange({
			measurementId: this.#curMsmIdx,
			measurement: {
				type,
				...msmConfig
			}
		});
		const { downloadApiUrl, uploadApiUrl, estimatedServerTime } = this.#config;
		let engine;
		switch (type) {
			case "v4Reachability":
			case "v6Reachability":
				engine = new ReachabilityEngine(`https://${msmConfig.host}`, { fetchOptions: {
					method: "GET",
					mode: "no-cors"
				} });
				engine.onFinished = (result) => {
					const r = result;
					msmResults.finished = true;
					msmResults.results = {
						host: msmConfig.host,
						...r
					};
					this.onResultsChange({ type });
					this.#next();
				};
				break;
			case "rpki":
				engine = new ReachabilityEngine(`https://${this.#config.rpkiInvalidHost}`, { timeout: 5e3 });
				engine.onFinished = (result) => {
					const r = result;
					(r.response ? r.response.json() : Promise.resolve()).then((response) => {
						msmResults.finished = true;
						msmResults.results = {
							host: this.#config.rpkiInvalidHost,
							filteringInvalids: !r.reachable,
							...response ? {
								asn: response.asn,
								name: response.name
							} : {}
						};
						this.onResultsChange({ type });
						this.#next();
					});
				};
				break;
			case "nxdomain":
				engine = new ReachabilityEngine(`https://${msmConfig.nxhost}`, { fetchOptions: { mode: "no-cors" } });
				engine.onFinished = (result) => {
					const r = result;
					msmResults.finished = true;
					msmResults.results = {
						host: msmConfig.nxhost,
						reachable: r.reachable
					};
					this.onResultsChange({ type });
					this.#next();
				};
				break;
			case "packetLoss":
			case "packetLossUnderLoad":
				{
					msmResults.finished = false;
					const { numPackets: numMsgs, ...ptCfg } = msmConfig;
					const { turnServerUri, turnServerCredsApiUrl: turnServerCredsApi, turnServerUser, turnServerPass, includeCredentials } = this.#config;
					try {
						engine = new PacketLossUnderLoadEngine({
							turnServerUri: turnServerUri ?? void 0,
							turnServerCredsApi: turnServerCredsApi ?? void 0,
							turnServerCredsApiIncludeCredentials: includeCredentials,
							turnServerUser: turnServerUser ?? void 0,
							turnServerPass: turnServerPass ?? void 0,
							authorization: this.authorization,
							numMsgs,
							downloadChunkSize: msmConfig.loadDown ? this.#optimalDownloadChunkSize : void 0,
							uploadChunkSize: msmConfig.loadUp ? this.#optimalUploadChunkSize : void 0,
							downloadApiUrl,
							uploadApiUrl,
							...ptCfg
						});
					} catch (e) {
						const message = e instanceof Error ? e.message : String(e);
						const measurementId = this.#measurementId;
						queueMicrotask(() => {
							if (measurementId !== this.#measurementId) return;
							msmResults.started = true;
							msmResults.finished = true;
							msmResults.error = message;
							this.onResultsChange({ type });
							this.#onError(`Error while measuring packet loss: ${message}`);
							this.#running && this.#next();
						});
						return;
					}
				}
				engine.onMsgReceived = () => {
					msmResults.results = Object.assign({}, engine.results);
					this.onResultsChange({ type });
				};
				engine.onFinished = () => {
					msmResults.finished = true;
					this.onResultsChange({ type });
					this.#next();
				};
				engine.onConnectionError = (e) => {
					msmResults.finished = true;
					msmResults.error = e;
					this.onResultsChange({ type });
					this.#onError(`Connection error while measuring packet loss: ${e}`);
					this.#next();
				};
				engine.onCredentialsFailure = () => {
					msmResults.finished = true;
					msmResults.error = "unable to get turn server credentials";
					this.onResultsChange({ type });
					this.#onError("Error while measuring packet loss: unable to get turn server credentials.");
					this.#next();
				};
				break;
			case "latency":
			case "latencyUnderLoad": {
				msmResults.finished = false;
				engine = new LoggingBandwidthEngine([{
					dir: "down",
					bytes: 0,
					count: msmConfig.numPackets,
					bypassMinDuration: true
				}], {
					downloadApiUrl,
					uploadApiUrl,
					estimatedServerTime,
					serverTimeDelta: this.#serverTimeDelta,
					logApiUrl: this.#config.logMeasurementApiUrl ?? void 0,
					measurementId: this.#measurementId,
					sessionId: this.#config.sessionId,
					authorization: this.authorization,
					downloadChunkSize: msmConfig.loadDown ? this.#optimalDownloadChunkSize : void 0,
					uploadChunkSize: msmConfig.loadUp ? this.#optimalUploadChunkSize : void 0
				});
				engine.fetchOptions = this.#config.includeCredentials ? { credentials: "include" } : {};
				engine.abortRequestDuration = this.#config.bandwidthAbortRequestDuration;
				if (!msmConfig.loadDown && !msmConfig.loadUp) {
					const eng = engine;
					eng.qsParams = {
						...eng.qsParams,
						during: "idle"
					};
				}
				const priorTimings = (msmResults.results?.timings || []).slice();
				engine.onMeasurementResult = engine.onNewMeasurementStarted = (_meas, results) => {
					msmResults.results = Object.assign({}, results.down[0]);
					msmResults.results.timings = priorTimings.concat(msmResults.results.timings);
					this.onResultsChange({ type });
				};
				engine.onFinished = () => {
					this.#serverTimeDelta = engine.serverTimeDelta;
					msmResults.finished = true;
					this.onResultsChange({ type });
					this.#running && this.#next();
				};
				engine.onConnectionError = (e, status) => {
					msmResults.error = e;
					this.#failed = true;
					this.pause();
					this.onResultsChange({ type });
					this.#onError(String(e), status);
				};
				engine.play();
				break;
			}
			case "download":
			case "upload": if (msmResults.finished || msmResults.error) this.#next();
			else {
				delete msmResults.finishedCurrentRound;
				const measureParallelLatency = this.#config[`measure${type === "download" ? "Down" : "Up"}loadLoadedLatency`];
				engine = new LoggingBandwidthEngine([{
					dir: type === "download" ? "down" : "up",
					...msmConfig
				}], {
					downloadApiUrl,
					uploadApiUrl,
					estimatedServerTime,
					serverTimeDelta: this.#serverTimeDelta,
					logApiUrl: this.#config.logMeasurementApiUrl ?? void 0,
					measurementId: this.#measurementId,
					measureParallelLatency,
					parallelLatencyThrottleMs: this.#config.loadedLatencyThrottle,
					sessionId: this.#config.sessionId,
					authorization: this.authorization
				});
				engine.fetchOptions = this.#config.includeCredentials ? { credentials: "include" } : {};
				engine.finishRequestDuration = this.#config.bandwidthFinishRequestDuration;
				engine.abortRequestDuration = this.#config.bandwidthAbortRequestDuration;
				engine.onNewMeasurementStarted = (...args) => {
					const { count, bytes } = args[0];
					const res = msmResults.results = Object.assign({}, msmResults.results);
					!res.hasOwnProperty(bytes) && (res[bytes] = {
						timings: [],
						numMeasurements: 0,
						sideLatency: measureParallelLatency ? [] : void 0
					});
					const bucket = res[bytes];
					if (bucket.numMeasurements - bucket.timings.length !== count) {
						bucket.numMeasurements += count;
						this.onResultsChange({ type });
					}
				};
				engine.onMeasurementResult = (...args) => {
					const { bytes, ...timing } = args[0];
					msmResults.results[bytes].timings.push(timing);
					msmResults.results = Object.assign({}, msmResults.results);
					this.onResultsChange({ type });
				};
				engine.onParallelLatencyResult = (res) => {
					msmResults.results[msmConfig.bytes].sideLatency.push(res);
					msmResults.results = Object.assign({}, msmResults.results);
					this.onResultsChange({ type });
				};
				engine.onFinished = (results) => {
					this.#serverTimeDelta = engine.serverTimeDelta;
					const bwResults = results;
					const isLastMsmOfType = !this.#config.measurements.slice(this.#curMsmIdx + 1).map((d) => d.type).includes(type);
					const durations = Object.values(type === "download" ? bwResults.down : bwResults.up).slice(-1)[0].timings.map((d) => d.duration);
					if (!(isLastMsmOfType || !msmConfig.bypassMinDuration && durations.length > 0 && Math.min(...durations) > this.#config.bandwidthFinishRequestDuration)) msmResults.finishedCurrentRound = true;
					else {
						msmResults.finished = true;
						this.onResultsChange({ type });
						const optimalSize = Object.keys(msmResults.results).map((n) => +n).sort((a, b) => b - a)[0] * OPTIMAL_SIZE_RATIO;
						type === "download" && (this.#optimalDownloadChunkSize = optimalSize);
						type === "upload" && (this.#optimalUploadChunkSize = optimalSize);
					}
					this.#running && this.#next();
				};
				engine.onConnectionError = (e, status) => {
					msmResults.error = e;
					this.#failed = true;
					this.pause();
					this.onResultsChange({ type });
					this.#onError(String(e), status);
				};
				engine.play();
			}
		}
		this.#curEngine = engine;
		msmResults.started = true;
		this.onResultsChange({ type });
	}
};
var SpeedTestEngine = class extends MeasurementEngine {
	constructor(userConfig = {}) {
		super(userConfig);
		super.onFinish = this.#logFinalResults;
	}
	set onFinish(onFinish) {
		super.onFinish = (results) => {
			onFinish(results);
			this.#logFinalResults(results);
		};
	}
	onResultsLogged = () => {};
	#logFinalResults = (results) => {
		const apiUrl = this.config.logAimApiUrl;
		if (!apiUrl) return;
		logAimResults(results, {
			apiUrl,
			includeCredentials: this.config.includeCredentials,
			sessionId: this.config.sessionId,
			authorization: this.authorization
		}).then((response) => {
			this.onResultsLogged(response);
		});
	};
};
//#endregion
export { SpeedTestEngine as default };

//# sourceMappingURL=speedtest.js.map