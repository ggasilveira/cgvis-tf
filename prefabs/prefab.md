# Prefabs file format

For each prefab we want to use in the level file, we
have to create a `toml` file for it.

## model

Type: `string`
Path to the `.glb` model file, relative to the `asset_dir` configured
in the code.

## mobility

Type: `"static" | "dynamic"`
Whether this prefab is static (part of the map) or dynamic (moves/interacts).

## colliders

Type: `"model"`
Use this key if the colliders are defined as meshes in the `.glb` model file.
The meshes/nodes must have names starting with `COL_`.

## `[[collider]]`

Type : `table`
Attach a collider to this prefab. Mutually exclusive with the `colliders` key.

### shape

Type: `"sphere" | "box" | "cylinder" | "capsule"`
Shape of the collider

### radius

Type: `float`
Radius of the sphere/cylinder/capsule

### height

Type: `float`
Height of the cylinder/capsule

### half_extents

Type: `[float; 3]`
Imagining a 3D cross inside the box, the size of each
axis of the cross (X, Y, Z) divided by two.

### offset

Type: `[float; 3]`
Offset of the collider with respect to the model

## interact

Type: `table`
Register this prefab as interactable. TODO.

### kind

Type: `"door" | "page"`
