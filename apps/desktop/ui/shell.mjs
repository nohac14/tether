const invoke = window.__TAURI__.core.invoke;
const byId = (id) => document.getElementById(id);
const status = byId("status");
const select = byId("network");
const port = byId("port");
const setup = byId("setup");
const pair = byId("pair");
const start = byId("start");
const autoListen = byId("auto-listen");
const autoListenHelp = byId("auto-listen-help");
const qr = byId("qr");
const notice = byId("notice");
const devices = byId("devices");
const pairState = byId("pair-state");
let state;
let pendingRequests = "";
let displayedCode;
let busy = false;
let autoListenKey;

const phaseLabels = {
  startup: "Starting",
  choose: "Ready to connect",
  starting: "Starting control",
  listening: "Listening on your private network",
  stopping: "Stopping control",
  unavailable: "Control unavailable",
};

function report(message) {
  notice.hidden = !message;
  notice.textContent = message ?? "";
}

async function command(name, args) {
  if (busy && name !== "stop_control") return;
  busy = true;
  try {
    await invoke(name, args);
    report();
  } catch {
    report("Tether could not complete that request. Try again.");
  } finally {
    busy = false;
  }
}

function updateNetworks(networks) {
  const earlier = select.value;
  const options = networks.map(({ index, label, address }) => {
    const option = document.createElement("option");
    option.value = String(index);
    option.textContent = label + " · " + address;
    return option;
  });
  select.replaceChildren(...options);
  if (options.some((option) => option.value === earlier)) select.value = earlier;
}

function updateAutoListen(view) {
  const network = view.networks.find(({ index }) => String(index) === select.value);
  const available = network?.autoListenAvailable === true;
  const key = view.revision + "/" + select.value + "/" + available;
  if (key !== autoListenKey) {
    autoListen.checked = network?.autoListenSaved === true;
    autoListenKey = key;
  }
  autoListen.disabled = !available;
  autoListenHelp.textContent = available
    ? "Applies after you start the listener and pair a phone."
    : "Automatic listening needs a verified private IPv4 network. You can still start it manually.";
}

function showCode(data) {
  if (data?.bits === displayedCode) return;
  displayedCode = data?.bits;
  qr.hidden = true;
  const context = qr.getContext("2d");
  if (!context) return;
  if (
    !data ||
    !Number.isInteger(data.width) ||
    data.width < 21 ||
    data.width > 177 ||
    data.bits.length !== data.width * data.width
  ) {
    context.clearRect(0, 0, qr.width, qr.height);
    return;
  }
  const module = 5;
  const quiet = 4;
  const size = (data.width + 2 * quiet) * module;
  qr.width = size;
  qr.height = size;
  context.fillStyle = "#fff";
  context.fillRect(0, 0, size, size);
  context.fillStyle = "#000";
  for (let row = 0; row < data.width; row += 1) {
    for (let column = 0; column < data.width; column += 1) {
      if (data.bits[row * data.width + column] === "1") {
        context.fillRect((quiet + column) * module, (quiet + row) * module, module, module);
      }
    }
  }
  qr.hidden = false;
}

function updateApprovals(view) {
  const keys = view.generation + "/" + view.pending.map(({ id }) => id).join(",");
  if (keys === pendingRequests) return;
  pendingRequests = keys;
  devices.replaceChildren(
    ...view.pending.map(({ id, label }) => {
      const row = document.createElement("li");
      const name = document.createElement("span");
      name.textContent = label;
      row.append(name);
      for (const approve of [true, false]) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = approve ? "Approve" : "Deny";
        if (!approve) button.className = "secondary";
        button.addEventListener("click", () => {
          void command("decide_phone", { generation: view.generation, request: id, approve });
        });
        row.append(button);
      }
      return row;
    }),
  );
}

function display(view) {
  status.textContent = view.controlling ? "Phone in control" : phaseLabels[view.phase];
  setup.hidden = view.phase !== "choose";
  pair.hidden = view.phase !== "listening";
  byId("phone-address").hidden = !view.listeningAddress;
  byId("listener-url").textContent = view.listeningAddress ?? "";
  byId("approvals").hidden = !view.pending.length;
  byId("paired").hidden = view.paired === 0;
  byId("paired-count").textContent =
    view.paired + (view.paired === 1 ? " paired phone" : " paired phones");
  if (state?.revision !== view.revision) updateNetworks(view.networks);
  if (state?.port !== view.port) {
    port.replaceChildren(
      ...Array.from(new Set([...view.ports, view.port]), (number) => {
        const option = document.createElement("option");
        option.value = String(number);
        option.textContent = option.value;
        return option;
      }),
    );
    port.value = String(view.port);
  }
  start.disabled = !view.networks.length;
  updateAutoListen(view);
  if (!view.networks.length && view.phase === "choose" && !view.notice) {
    report("No private Wi-Fi or Ethernet network is available. Refresh after connecting.");
  } else if (view.notice !== state?.notice || view.revision !== state?.revision) {
    report(view.notice);
  }
  pairState.textContent = view.qr
    ? ""
    : view.connected
      ? view.connected + " connected. Generate a new code to pair another phone."
      : "Generate a new code to pair a phone.";
  showCode(view.qr);
  updateApprovals(view);
  state = view;
}

start.addEventListener("click", () => {
  if (state && select.value !== "" && port.value !== "") {
    void command("start_network", {
      revision: state.revision,
      index: Number(select.value),
      port: Number(port.value),
      autoListen: autoListen.checked && !autoListen.disabled,
    });
  }
});
select.addEventListener("change", () => {
  if (state) updateAutoListen(state);
});
byId("refresh").addEventListener("click", () => {
  void command("refresh_networks");
});
byId("stop").addEventListener("click", () => {
  void command("stop_control");
});
byId("offer").addEventListener("click", () => {
  void command("new_code");
});
byId("forget").addEventListener("click", () => {
  if (window.confirm("Forget every paired phone and stop control?")) void command("revoke_phones");
});

async function update() {
  try {
    display(await invoke("snapshot"));
  } catch {
    report("Tether could not reach the local control service.");
  }
  window.setTimeout(() => {
    void update();
  }, 350);
}

const startup = byId("startup");
const startupNotice = byId("startup-notice");
let checkingStartup = false;
async function syncStartup(change) {
  if (checkingStartup) return;
  checkingStartup = true;
  startup.disabled = true;
  try {
    const registration = await (change
      ? invoke("set_autostart", { enabled: startup.checked })
      : invoke("get_autostart_state"));
    startup.indeterminate = false;
    startup.checked = registration.enabled;
    startupNotice.hidden = !registration.blocked;
    startupNotice.textContent =
      "Windows has disabled Tether. To run it at sign-in, enable Tether under Settings → Apps → Startup.";
  } catch (error) {
    // Show an unknown state if Windows could not confirm the registration.
    startup.checked = false;
    startup.indeterminate = true;
    startupNotice.textContent = String(error);
    startupNotice.hidden = false;
  } finally {
    startup.disabled = false;
    checkingStartup = false;
  }
}
startup.addEventListener("change", () => void syncStartup(true));
window.addEventListener("focus", () => void syncStartup(false));
void syncStartup(false);
void update();
