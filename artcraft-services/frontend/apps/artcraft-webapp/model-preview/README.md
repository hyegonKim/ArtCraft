# ArtCraft model preview

This standalone web build uses the model definitions in the repository.
Both model pickers work without an API, and Generate is disabled.

From `artcraft-services/frontend` after installing the workspace dependencies:

```sh
npm exec -- vite build apps/artcraft-webapp/model-preview
npm exec -- vite preview apps/artcraft-webapp/model-preview
```

Open the URL printed by Vite. The image and video pages are at
`/#/create-image` and `/#/create-video`. Publish only
`apps/artcraft-webapp/model-preview/dist` to a static host. Relative assets and
hash routes let the preview work under a project subpath.
