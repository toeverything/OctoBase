const { Storage } = require(".");

void (async () => {
  const storage = await Storage.connect("sqlite:./test.db?mode=rwc");

  const workspace = await storage.getOrCreateWorkspace("first workspace");

  console.log(workspace.clientId(), workspace.id());
})();
