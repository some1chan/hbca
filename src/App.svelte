<script lang="ts">
	import QRCode from "@bonosoft/sveltekit-qrcode";
	import { MetaTags } from "svelte-meta-tags";
	import * as Tone from "tone";
	import tickUrl from "/src/audio/tick.ogg?url";
	import tockUrl from "/src/audio/tock.ogg?url";

	let rpcVersion = 0;
	enum Packet {
		PeerVersion = "PeerVersion",
		Ping = "Ping",
		Pong = "Pong",
		FatalError = "FatalError",
		SendCountdown = "SendCountdown",
	}

	const urlParams = new URLSearchParams(window.location.search);

	// QR Code
	let showQrCode = false;
	let scale = 5;
	let time = new Date().getTime();
	let friendlyTime = "";

	function updateTime() {
		time = new Date().getTime();

		const date = new Date(time);
		const hours = date.getUTCHours().toString().padStart(2, "0");
		const minutes = date.getUTCMinutes().toString().padStart(2, "0");
		const seconds = date.getUTCSeconds().toString().padStart(2, "0");
		const milliseconds = date.getMilliseconds().toString().padStart(3, "0");
		friendlyTime = `${hours}:${minutes}:${seconds}.${milliseconds}`;

		requestAnimationFrame(updateTime);
	}
	// let updateTimeId = requestAnimationFrame(updateTime);
	// cancelAnimationFrame(updateTimeId);

	// Connecting
	import { type DataConnection, Peer } from "peerjs";
	import { invoke } from "@tauri-apps/api/tauri";

	let selfPeerId = urlParams.get("id");

	// Remove eventually:tm:
	// const hardCodedId = "f84fc078-001d-4aad-90eb-059840ad798d";
	// selfPeerId = window.__TAURI_IPC__ == undefined ? hardCodedId : "";

	let peerToConnectTo = urlParams.get("connect-to") ?? "";
	// peerToConnectTo = window.__TAURI_IPC__ != undefined ? hardCodedId : "";

	let username: string = urlParams.get("username") ?? "Beat";
	let isClient: boolean =
		(urlParams.get("client") != undefined // If client's defined
			? urlParams.get("client").toLowerCase() == "true"
				? true // If set to true, set to true
				: urlParams.get("client").toLowerCase() == "false"
				? false // If set to false, set to false
				: undefined // Pass it along to the window IPC check
			: undefined) ?? true;
	let copied = false;
	let peer: Peer;
	let peerText: HTMLInputElement;
	let connections = new Map<string, DataConnection>();
	let heartbeatConnections = new Map<
		string,
		{
			pingInterval?: NodeJS.Timer;
			data: {
				timeToSend: number;
				timeToRespond: number | undefined;
				sequence: number;
			}[];
		}
	>();
	getPeer();

	function getPeer() {
		peer = new Peer(selfPeerId, {
			config: {
				iceServers: [{ url: "stun:stun.l.google.com:19302" }],
			},
		});

		// Register with the peer server
		peer.on("open", function (id) {
			console.log("Peer ID: " + id);
			selfPeerId = peer.id;
		});
		peer.on("error", async function (error) {
			console.error(error);
			if (error.message.includes("is taken")) {
				peer.destroy();
				selfPeerId = "";
				getPeer();
			}
		});
		peer.on("disconnected", function () {
			peer.reconnect();
		});
		peer.on("close", () => {
			// Just in case
			for (const [, connection] of connections) {
				connection.close();
			}
			for (const [, heartbeat] of heartbeatConnections) {
				clearInterval(heartbeat.pingInterval);
			}
			// Then clear
			connections = new Map();
			heartbeatConnections = new Map();
		});

		// Handle incoming data
		peer.on("connection", (connection) => {
			console.log(`incoming peer connection!`);
			connection.on("open", () => {
				// Version check: it's the responsibility of the client to deny requests
				connection.send({
					command: Packet.PeerVersion,
					rpcVersion: rpcVersion,
				});

				const oldConnection = connections.get(connection.peer);
				if (oldConnection) {
					oldConnection.close();
				}

				connections.set(connection.peer, connection);
				// connection.send("hello!");

				// Heartbeat
				let sequence = 0;
				const oldHeartbeatConnections = heartbeatConnections.get(
					connection.connectionId
				);
				if (oldHeartbeatConnections) {
					heartbeatConnections.delete(connection.connectionId);
				}
				heartbeatConnections.set(connection.connectionId, { data: [] });
				const pingTracking = heartbeatConnections.get(connection.connectionId);
				const pingInterval = setInterval(() => {
					pingTracking.pingInterval = pingInterval;
					pingTracking.data.push({
						timeToSend: performance.now(),
						timeToRespond: undefined,
						sequence: sequence,
					});
					connection.send({
						command: Packet.Ping,
						sequence: sequence,
					});
					sequence++;
					if (
						pingTracking.data.filter((p) => p.timeToRespond == undefined)
							.length > 5
					) {
						console.log(
							`Connection ${connection.metadata["username"]} (${connection.peer}) hasn't responded to 5 pings, disconnecting`
						);
						heartbeatConnections.delete(connection.connectionId);
						// connection.close();
						clearInterval(pingInterval);
					}
					connections = connections;
				}, 1_000);
			});
			connection.on("data", (data) => {
				if (data["command"] == Packet.Ping) {
					connection.send({ command: Packet.Pong, sequence: data["sequence"] });
					return;
				} else if (data["command"] == Packet.Pong) {
					// connection.send({ command: Packet.Pong, sequence: data["sequence"] });
					const pingTracking = heartbeatConnections.get(
						connection.connectionId
					);
					pingTracking.data.find(
						(p) => p.sequence == data["sequence"]
					).timeToRespond = performance.now();
					if (pingTracking.data.length >= 10) {
						pingTracking.data.shift();
					}
					return;
				}

				console.log(`Received data from incoming:`);
				console.log(data);

				if (
					data["command"] == Packet.PeerVersion &&
					data["rpcVersion"] > rpcVersion
				) {
					connection.send({
						command: Packet.FatalError,
						error: `Incompatible host version ${data["rpcVersion"]} (currently on ${rpcVersion})`,
					});
					connection.close();
					return;
				}
			});
			connection.on("close", () => {
				connections.delete(connection.peer);
				connections = connections;

				// Duplication
				const heartbeat = heartbeatConnections.get(connection.connectionId);
				if (heartbeat) {
					clearInterval(heartbeat.pingInterval);
					heartbeatConnections.delete(connection.connectionId);
				}
			});
		});
	}

	// Initiate outgoing connection
	let outgoingConnection: DataConnection | undefined;
	async function connectToPeer() {
		console.time("Audio ready");
		await Tone.start();
		console.timeEnd("Audio ready");

		if (outgoingConnection) {
			outgoingConnection.close();
			return;
		}

		console.log(`Connecting to ${peerToConnectTo}...`);

		let connection = peer.connect(peerToConnectTo, {
			metadata: { username: username },
		});
		outgoingConnection = connection;

		connection.on("open", () => {
			connections.set(connection.peer, connection);
			// connections = connections;
			outgoingConnection = connection;

			// connection.send("hi!");
		});
		connection.on("data", (data) => {
			if (data["command"] == Packet.Ping) {
				connection.send({ command: Packet.Pong, sequence: data["sequence"] });
				return;
			}

			console.log(`Received data from outgoing:`);
			console.log(data);

			const command = data["command"];
			if (command == Packet.SendCountdown) {
				performLocalCountdown(true);
				return;
			}
		});
		connection.on("close", () => {
			connections.delete(connection.peer);
			connections = connections;
			outgoingConnection = undefined;
		});
		connection.on("error", console.error);
	}

	// Timing
	const metronomeSampler = new Tone.Sampler({
		urls: {
			G6: tickUrl,
			"C#6": tockUrl,
		},
	}).toDestination();

	let metronomeVolume = urlParams.get("volume") ?? "50";
	$: {
		// https://github.com/audiojs/decibels/blob/master/from-gain.js
		const volumePercent = Number(metronomeVolume) / 100;
		const volumeDecibels = 20 * (0.43429 * Math.log(volumePercent));
		metronomeSampler.volume.value = volumeDecibels;
		console.log(
			`Adjusted volume to ${metronomeVolume}% (${metronomeSampler.volume.value} dB)`
		);
	}
	let metronomeIsPlaying = false;
	let visualCountdown = "";

	function performLocalCountdown(input = false) {
		const now = Tone.now();
		let index: number;
		for (index = 0; index < 4; index++) {
			// if (index == 0) continue;
			metronomeSampler.triggerAttackRelease(
				index == 0 ? "G6" : "C#6",
				"8n",
				now + index
			);
		}
		metronomeSampler.triggerAttackRelease("G6", "8n", now + index);

		metronomeIsPlaying = true;
		visualCountdown = "4";
		setTimeout(() => {
			visualCountdown = "3";
		}, 1_000);
		setTimeout(() => {
			visualCountdown = "2";
		}, 2_000);
		setTimeout(() => {
			visualCountdown = "1";
		}, 3_000);
		setTimeout(async () => {
			visualCountdown = "Go!";
			if (input) {
				const validHandle = await focusWindow();
				if (validHandle) await sendInput();
			}
		}, 4_000);
		setTimeout(() => {
			visualCountdown = "";
			metronomeIsPlaying = false;
		}, 4_500);
	}

	async function focusWindow() {
		if (window.__TAURI_IPC__ == undefined) return "0x0";
		const handleString: string = await invoke("focus_window", {
			windowName: "UNBEATABLE [white label]",
		});
		return handleString != "0x0";
	}

	const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

	async function sendInput(delay = 0) {
		if (delay > 0) await sleep(delay);
		if (window.__TAURI_IPC__ == undefined) {
			console.log("Skipping pressing the key, Tauri IPC doesn't exist");
			return;
		}
		console.log("Pressing key");
		const result = await invoke("press_key", {
			key: "f",
		});
		if (result) {
			console.error(result);
		}
	}

	async function broadcastCountdown() {
		metronomeIsPlaying = true;
		console.time("Audio ready");
		await Tone.start();
		console.timeEnd("Audio ready");

		// Get the largest ping
		let largestPing = -1;
		for (const [, connection] of connections) {
			const ping = getPing(connection).recentPing;
			largestPing = Math.max(ping, largestPing);
		}

		// Send out packets for countdowns on delayed timings
		for (const [, connection] of connections) {
			// Connection with the largest amount of latency
			// has its command sent out basically instantly,
			// and everything else gets delayed based on it.
			//
			// See bottom comment about round-trip time for
			// why we're dividing by two after our subtraction.
			const ping = getPing(connection).recentPing;
			console.log(`${connection.peer} - ${largestPing}-${ping}`);
			setTimeout(() => {
				connection.send({
					command: Packet.SendCountdown,
				});
			}, (largestPing - ping) / 2);
		}

		// Ping is the round-trip time, and this assumes that the same amount
		// of time sending data from server to client, is the same as client to
		// server. We're only concerned about server to client here,
		// so dividing by two is our only way to guess this.
		await sleep(largestPing / 2);

		performLocalCountdown();
	}

	function getPingVisual(connection: DataConnection) {
		const pingData = getPing(connection);
		return (
			`${pingData.recentPing}ms` +
			(pingData.failedToRespondCount > 0
				? ` (failed ${pingData.failedToRespondCount} times)`
				: "")
		);
	}

	function getPing(connection: DataConnection) {
		let failedToRespondCount = 0;
		let index = heartbeatConnections.size - 1;
		const heartbeatConnectionsValues = Array.from(
			heartbeatConnections.get(connection.connectionId).data.values()
		);
		console.log(heartbeatConnectionsValues);
		for (index = index; index >= 0; index--) {
			const pingData = heartbeatConnectionsValues[index];
			if (pingData?.timeToRespond == undefined) {
				failedToRespondCount++;
				continue;
			}
			break;
		}

		// Can be undefined, as the index can be -1 if no pings were previously done
		const pingData = heartbeatConnectionsValues[index];
		console.log(pingData);
		const recentPing = pingData
			? pingData.timeToRespond - pingData.timeToSend
			: -1;

		return {
			recentPing,
			failedToRespondCount,
		};
	}
</script>

<MetaTags
	title="high budget control application"
	description="A very high budget control application for synchronizing inputs."
	additionalMetaTags={[{ name: "theme-color", content: "#1a56db" }]}
	openGraph={{
		type: "website",
		url: "https://hbca.pages.dev/",
		title: "High Budget Control Application",
		description:
			"A very high budget control application for synchronizing inputs.",
	}}
/>

<main>
	<div class="m-2 mx-4">
		<div class="format py-6 select-none">
			<h1 class="mb-2">high budget control application™</h1>
			<!-- svelte-ignore missing-declaration -->
			<p class="select-text">
				You're currently running version {APP_VERSION} (git hash {COMMIT_HASH}).
			</p>
			<p>A few important things:</p>
			<ul>
				<li>
					Please follow the <a
						href="https://gist.github.com/some1chan/ffbe864f7c2b815525677e846d1ecbc9/"
						class="text-blue-600 dark:text-blue-500 hover:underline"
						target="_blank"
						rel="noreferrer">guide</a
					> on what to do.
				</li>
				<li>
					<a
						href="https://github.com/some1chan/hbca/releases"
						class="text-blue-600 dark:text-blue-500 hover:underline"
						target="_blank"
						rel="noreferrer">Downloading the app</a
					>
					is preferable to the webapp.
				</li>
				<li>
					Also, when stuff breaks (ex. you can't connect), use
					<code>Ctrl+R</code>
					to reload.
				</li>
			</ul>
			<p>- some1chan</p>
		</div>
		<form
			class="select-none max-w-2xl"
			on:submit|preventDefault={connectToPeer}
		>
			<div class="flex select-none mb-4">
				<div class="flex items-center h-5">
					<input
						id="controller"
						type="checkbox"
						value=""
						class="w-4 h-4 border border-gray-300 rounded bg-gray-50 focus:ring-3 focus:ring-blue-300 dark:bg-gray-700 dark:border-gray-600 dark:focus:ring-blue-600 dark:ring-offset-gray-800"
						bind:checked={isClient}
						disabled={connections.size >= 1}
					/>
				</div>
				<div class="ml-2 text-sm">
					<label
						for="controller"
						class="text-sm font-medium text-gray-900 dark:text-gray-300"
						>Is Client</label
					>
					<p
						id="controller"
						class="text-xs font-normal text-gray-500 dark:text-gray-300"
					>
						This should be checked if you will have your game controlled by
						someone, or are controlling completely manually and need accurate
						timing on when to start.
					</p>
				</div>
			</div>
			<label
				for="peer-id"
				class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
				>Your {!isClient ? "Host " : ""}ID</label
			>
			<div class="flex mb-4 gap-2">
				<input
					type={isClient ? "password" : "text"}
					class="bg-gray-100 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 cursor-not-allowed dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-gray-400 dark:focus:ring-blue-500 dark:focus:border-blue-500"
					placeholder="Getting your ID..."
					value={selfPeerId}
					readonly
					disabled
					bind:this={peerText}
					on:blur={() => {
						// copied = false;
					}}
				/>
				<button
					type="button"
					class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm p-2.5 text-center inline-flex items-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
					disabled={selfPeerId == ""}
					on:click={() => {
						peerText.select();
						navigator.clipboard.writeText(selfPeerId);
						copied = true;
					}}
					on:blur={() => {
						copied = false;
					}}
				>
					{#if selfPeerId == ""}
						<svg
							aria-hidden="true"
							role="status"
							class="inline w-5 h-5 text-white animate-spin"
							viewBox="0 0 100 101"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z"
								fill="#E5E7EB"
							/>
							<path
								d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z"
								fill="currentColor"
							/>
						</svg>
					{:else if !copied}
						<svg
							aria-hidden="true"
							class="w-5 h-5"
							fill="none"
							stroke="currentColor"
							stroke-width="1.5"
							viewBox="0 0 24 24"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M8.25 7.5V6.108c0-1.135.845-2.098 1.976-2.192.373-.03.748-.057 1.123-.08M15.75 18H18a2.25 2.25 0 002.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 00-1.123-.08M15.75 18.75v-1.875a3.375 3.375 0 00-3.375-3.375h-1.5a1.125 1.125 0 01-1.125-1.125v-1.5A3.375 3.375 0 006.375 7.5H5.25m11.9-3.664A2.251 2.251 0 0015 2.25h-1.5a2.251 2.251 0 00-2.15 1.586m5.8 0c.065.21.1.433.1.664v.75h-6V4.5c0-.231.035-.454.1-.664M6.75 7.5H4.875c-.621 0-1.125.504-1.125 1.125v12c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V16.5a9 9 0 00-9-9z"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					{:else}
						<svg
							aria-hidden="true"
							class="w-5 h-5"
							fill="none"
							stroke="currentColor"
							stroke-width="1.5"
							viewBox="0 0 24 24"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M11.35 3.836c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 00.75-.75 2.25 2.25 0 00-.1-.664m-5.8 0A2.251 2.251 0 0113.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m8.9-4.414c.376.023.75.05 1.124.08 1.131.094 1.976 1.057 1.976 2.192V16.5A2.25 2.25 0 0118 18.75h-2.25m-7.5-10.5H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V18.75m-7.5-10.5h6.375c.621 0 1.125.504 1.125 1.125v9.375m-8.25-3l1.5 1.5 3-3.75"
								stroke-linecap="round"
								stroke-linejoin="round"
							/>
						</svg>
					{/if}
					<!-- <svg
					aria-hidden="true"
					class="w-5 h-5"
					fill="currentColor"
					viewBox="0 0 20 20"
					xmlns="http://www.w3.org/2000/svg"
					><path
						fill-rule="evenodd"
						d="M10.293 3.293a1 1 0 011.414 0l6 6a1 1 0 010 1.414l-6 6a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-4.293-4.293a1 1 0 010-1.414z"
						clip-rule="evenodd"
					/></svg
				> -->
					<span class="sr-only">Icon description</span>
				</button>
			</div>

			{#if isClient}
				<div class="grid gap-6 md:grid-cols-2">
					<div class="mb-4">
						<label
							for="peer-id"
							class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
							>Peer to Connect To</label
						>
						<input
							type="text"
							id="peer-id"
							class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
							placeholder="00000000-0000-0000-0000-000000000000"
							autocomplete="off"
							required
							bind:value={peerToConnectTo}
						/>
					</div>
					<div class="mb-4">
						<label
							for="username"
							class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
							>Username</label
						>
						<input
							type="text"
							id="username"
							class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500"
							placeholder="Beat"
							autocomplete="off"
							required
							bind:value={username}
						/>
					</div>
				</div>
			{/if}

			<div class="flex flex-row gap gap-2">
				{#if isClient}
					<button
						type="submit"
						class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm w-full sm:w-auto px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
						disabled={outgoingConnection && !outgoingConnection.open}
						>{!outgoingConnection
							? "Connect"
							: !outgoingConnection.open
							? "Connecting..."
							: "Disconnect"}</button
					>
				{/if}
				<!-- <button
					type="button"
					class="py-2.5 px-5 text-sm w-full sm:w-auto font-medium text-gray-900 focus:outline-none bg-white rounded-lg border border-gray-200 hover:bg-gray-100 hover:text-blue-700 focus:z-10 focus:ring-4 focus:ring-gray-200 dark:focus:ring-gray-700 dark:bg-gray-800 dark:text-gray-400 dark:border-gray-600 dark:hover:text-white dark:hover:bg-gray-700"
					>Open Stream Sync</button
				> -->
			</div>
		</form>

		{#if !isClient}
			<div class="format pt-10 pb-4 select-none">
				<h2 class="mb-0">Connections</h2>
			</div>
			{#if connections.size == 0}<p>None, currently</p>{/if}
			{#each Array.from(connections.values()) as connection}
				<p>
					{connection.metadata["username"]}: {getPingVisual(connection)} - {connection.peer}
				</p>
			{/each}
		{/if}

		<div class="format pt-10 pb-4 select-none">
			<h2 class="mb-0">Countdown</h2>
		</div>
		<div
			class="h-14 w-14 mb-3 {Number.isNaN(Number(visualCountdown)) ||
			visualCountdown == ''
				? 'bg-blue-500'
				: Number(visualCountdown) % 2 == 0
				? 'bg-red-500 opacity-50'
				: 'bg-red-500'}"
		/>
		<form class="select-none max-w-2xl">
			<p class="text-xl mb-2">
				Countdown: {visualCountdown}
			</p>
			<div class="mb-4">
				<label
					for="metronome-volume"
					class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
					>Volume ({metronomeVolume}%)</label
				>
				<input
					id="metronome-volume"
					type="range"
					class="w-72 h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700"
					bind:value={metronomeVolume}
				/>
			</div>
		</form>

		{#if !isClient}
			<div class="format pt-10 pb-4 select-none">
				<h2 class="mb-0">Control</h2>
			</div>
			<button
				type="submit"
				class="text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm w-full sm:w-auto px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
				disabled={metronomeIsPlaying}
				on:click={() => {
					broadcastCountdown();
				}}>Send Countdown</button
			>
		{/if}

		<div class="format pt-10 pb-4 select-none">
			<h2 class="mb-0">Testing</h2>
		</div>

		<form class="select-none max-w-xl" on:submit|preventDefault={connectToPeer}>
			<button
				type="button"
				class="text-white bg-blue-700 hover:bg-blue-800 active:bg-blue-900 focus:ring-4 focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 mb-2 dark:bg-blue-600 dark:hover:bg-blue-700 dark:active:bg-blue-800 focus:outline-none dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
				disabled={metronomeIsPlaying}
				on:click={async () => {
					console.time("Audio ready");
					await Tone.start();
					console.timeEnd("Audio ready");
					performLocalCountdown();
				}}>Audio Countdown</button
			>
			{#if isClient}
				<button
					type="button"
					class="text-white bg-blue-700 hover:bg-blue-800 active:bg-blue-900 focus:ring-4 focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 mb-2 dark:bg-blue-600 dark:hover:bg-blue-700 dark:active:bg-blue-800 focus:outline-none dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
					disabled={window.__TAURI_IPC__ == undefined}
					on:click={() => {
						focusWindow();
					}}>Focus Window</button
				>

				<button
					type="button"
					class="text-white bg-blue-700 hover:bg-blue-800 active:bg-blue-900 focus:ring-4 focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 mb-2 dark:bg-blue-600 dark:hover:bg-blue-700 dark:active:bg-blue-800 focus:outline-none dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
					disabled={window.__TAURI_IPC__ == undefined}
					on:click={async () => {
						await focusWindow();
						await sendInput();
					}}>Send Input to Window</button
				>

				<button
					type="button"
					class="text-white bg-blue-700 hover:bg-blue-800 active:bg-blue-900 focus:ring-4 focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 mb-2 dark:bg-blue-600 dark:hover:bg-blue-700 dark:active:bg-blue-800 focus:outline-none dark:focus:ring-blue-800 disabled:bg-blue-400 disabled:dark:bg-blue-500 disabled:cursor-not-allowed"
					disabled={metronomeIsPlaying || window.__TAURI_IPC__ == undefined}
					on:click={async () => {
						console.time("Audio ready");
						await Tone.start();
						console.timeEnd("Audio ready");
						performLocalCountdown(true);
					}}>Run Countdown</button
				>
			{/if}
		</form>

		{#if showQrCode}
			<div
				class="bg-neutral-100 dark:bg-neutral-900 flex items-center pt-[{scale}] select-none"
			>
				{#key time}
					<QRCode
						size={(23 * scale).toString()}
						padding="0"
						content={time.toString()}
						errorCorrection="L"
					/>
				{/key}
				<div class="text-5xl dark:text-white px-4 font-black tabular-nums">
					{friendlyTime}
				</div>
			</div>
		{/if}
	</div>
</main>
