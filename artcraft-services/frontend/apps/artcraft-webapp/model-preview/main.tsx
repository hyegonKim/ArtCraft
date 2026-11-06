import { useState } from "react";
import { createRoot } from "react-dom/client";
import { HashRouter, Navigate, NavLink, Route, Routes } from "react-router-dom";
import type { Model } from "../../../libs/model-list/src/lib/classes/Model.js";
import { IMAGE_MODELS } from "../../../libs/model-list/src/lib/lists/ImageModels.js";
import { VIDEO_MODELS } from "../../../libs/model-list/src/lib/lists/VideoModels.js";
import "./styles.css";

interface ModelScreenProps {
  title: string;
  models: readonly Model[];
  selectedId: string;
  onSelect: (id: string) => void;
}

function ModelScreen({ title, models, selectedId, onSelect }: ModelScreenProps) {
  const selected = models.find((model) => model.id === selectedId);

  return (
    <main>
      <h1>{title}</h1>
      <p id="preview-note">
        Browse the built-in model catalog. Generation is disabled in this preview.
      </p>
      <label htmlFor="preview-model">Model</label>
      <select
        id="preview-model"
        value={selectedId}
        onChange={(event) => onSelect(event.currentTarget.value)}
      >
        {models.map((model) => (
          <option key={model.id} value={model.id}>
            {model.fullName}
          </option>
        ))}
      </select>
      <p className="selection" aria-live="polite">
        Selected model: <strong>{selected?.fullName}</strong>
      </p>
      <button type="button" disabled aria-describedby="preview-note">
        Generate
      </button>
    </main>
  );
}

function ModelPreview() {
  const [imageId, setImageId] = useState(IMAGE_MODELS[0].id);
  const [videoId, setVideoId] = useState(VIDEO_MODELS[0].id);

  return (
    <HashRouter>
      <header>
        <strong>ArtCraft</strong>
        <span>Model preview</span>
        <nav aria-label="Create">
          <NavLink to="/create-image">Image</NavLink>
          <NavLink to="/create-video">Video</NavLink>
        </nav>
      </header>
      <Routes>
        <Route
          path="/create-image"
          element={
            <ModelScreen
              title="Create Image"
              models={IMAGE_MODELS}
              selectedId={imageId}
              onSelect={setImageId}
            />
          }
        />
        <Route
          path="/create-video"
          element={
            <ModelScreen
              title="Create Video"
              models={VIDEO_MODELS}
              selectedId={videoId}
              onSelect={setVideoId}
            />
          }
        />
        <Route path="*" element={<Navigate to="/create-image" replace />} />
      </Routes>
    </HashRouter>
  );
}

createRoot(document.getElementById("root")!).render(<ModelPreview />);
