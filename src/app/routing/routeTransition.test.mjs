import assert from "node:assert/strict";
import { test } from "node:test";
import { beginRouteTransition, completeRouteExit, createInitialRouteLayer } from "./routeTransition.ts";

function serializeLayers(layers) {
  return layers.map((layer) => ({
    key: layer.key,
    routeId: layer.route.id,
    phase: layer.phase,
  }));
}

const dashboardRoute = {
  id: "dashboard",
  path: "/",
  element: function DashboardRouteElement() {
    return null;
  },
};

const modsRoute = {
  id: "mods",
  path: "/mods",
  element: function ModsRouteElement() {
    return null;
  },
};

const profilesRoute = {
  id: "profiles",
  path: "/profiles",
  element: function ProfilesRouteElement() {
    return null;
  },
};

test("creates a stable initial route layer", () => {
  const layer = createInitialRouteLayer(dashboardRoute);

  assert.equal(layer.key, "dashboard:/");
  assert.equal(layer.route, dashboardRoute);
  assert.equal(layer.phase, "active");
});

test("beginRouteTransition keeps the old route exiting and the target route entering", () => {
  const currentLayers = [createInitialRouteLayer(dashboardRoute)];

  const nextLayers = beginRouteTransition(currentLayers, modsRoute);

  assert.deepEqual(serializeLayers(nextLayers), [
    { key: "dashboard:/", routeId: "dashboard", phase: "exiting" },
    { key: "mods:/mods", routeId: "mods", phase: "entering" },
  ]);
});

test("beginRouteTransition ignores navigation to the already visible route", () => {
  const currentLayers = [createInitialRouteLayer(dashboardRoute)];

  const nextLayers = beginRouteTransition(currentLayers, dashboardRoute);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "dashboard:/", routeId: "dashboard", phase: "active" }]);
});

test("beginRouteTransition replaces an in-flight entering route with the newest target", () => {
  const inFlightLayers = beginRouteTransition([createInitialRouteLayer(dashboardRoute)], modsRoute);

  const nextLayers = beginRouteTransition(inFlightLayers, dashboardRoute);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "dashboard:/", routeId: "dashboard", phase: "active" }]);
});

test("beginRouteTransition preserves an in-flight transition to the existing target route", () => {
  const inFlightLayers = beginRouteTransition([createInitialRouteLayer(dashboardRoute)], modsRoute);

  const nextLayers = beginRouteTransition(inFlightLayers, modsRoute);

  assert.deepEqual(serializeLayers(nextLayers), [
    { key: "dashboard:/", routeId: "dashboard", phase: "exiting" },
    { key: "mods:/mods", routeId: "mods", phase: "entering" },
  ]);
});

test("beginRouteTransition transitions from the newest visible layer", () => {
  const currentLayers = [
    { key: "dashboard:/", route: dashboardRoute, phase: "active" },
    { key: "mods:/mods", route: modsRoute, phase: "entering" },
  ];

  const nextLayers = beginRouteTransition(currentLayers, profilesRoute);

  assert.deepEqual(serializeLayers(nextLayers), [
    { key: "mods:/mods", routeId: "mods", phase: "exiting" },
    { key: "profiles:/profiles", routeId: "profiles", phase: "entering" },
  ]);
});

test("beginRouteTransition drops older exiting layers when switching to a third route", () => {
  const inFlightLayers = beginRouteTransition([createInitialRouteLayer(dashboardRoute)], modsRoute);

  const nextLayers = beginRouteTransition(inFlightLayers, profilesRoute);

  assert.deepEqual(serializeLayers(nextLayers), [
    { key: "mods:/mods", routeId: "mods", phase: "exiting" },
    { key: "profiles:/profiles", routeId: "profiles", phase: "entering" },
  ]);
});

test("completeRouteExit removes exiting layers and promotes entering route to active", () => {
  const inFlightLayers = beginRouteTransition([createInitialRouteLayer(dashboardRoute)], modsRoute);

  const nextLayers = completeRouteExit(inFlightLayers);

  assert.deepEqual(serializeLayers(nextLayers), [{ key: "mods:/mods", routeId: "mods", phase: "active" }]);
});

test("completeRouteExit keeps only the newest non-exiting layer", () => {
  const layers = [
    { key: "dashboard:/", route: dashboardRoute, phase: "exiting" },
    { key: "mods:/mods", route: modsRoute, phase: "exiting" },
    { key: "profiles:/profiles", route: profilesRoute, phase: "entering" },
  ];

  const nextLayers = completeRouteExit(layers);

  assert.deepEqual(serializeLayers(nextLayers), [
    { key: "profiles:/profiles", routeId: "profiles", phase: "active" },
  ]);
});
