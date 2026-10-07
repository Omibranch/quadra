"""Renders the startup cube: 2x2x2 small cubes of different brightness turning in eased
quarter turns. Run headless:

    blender -b --factory-startup -P tools/intro_cube.py -- <output dir> [--still]

Writes f_0000.png .. f_0095.png (RGBA, white-to-grey: the app tints them with the accent
colour). tools/intro_sheet.py packs them into src/assets/intro.webp.
"""
import math
import os
import sys

import bpy
from mathutils import Vector

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "/tmp/quadra_intro"
SIZE = 240
QUARTER = 24          # frames per quarter turn
MOVE = 20             # of which the cube is moving; the rest it stands still
FRAMES = QUARTER * 4  # a full turn, so the loop closes on the very same picture

# Brightness of the eight small cubes, placed so that every face shows four different ones.
SHADES = {
    (-1, -1, -1): 0.16, (1, -1, -1): 0.50, (-1, 1, -1): 0.78, (1, 1, -1): 0.30,
    (-1, -1, 1): 0.62, (1, -1, 1): 1.00, (-1, 1, 1): 0.24, (1, 1, 1): 0.40,
}


def ease(t):
    return 4 * t ** 3 if t < 0.5 else 1 - (-2 * t + 2) ** 3 / 2


def cube_angle(frame):
    """Angle of the whole cube at a (possibly fractional) frame: quarter turns, eased, then a rest."""
    frame = max(0.0, frame)
    quarter, k = divmod(frame, QUARTER)
    return (quarter + ease(min(1.0, k / MOVE))) * math.pi / 2


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene

    pivot = bpy.data.objects.new("pivot", None)
    scene.collection.objects.link(pivot)

    for (x, y, z), shade in SHADES.items():
        bpy.ops.mesh.primitive_cube_add(size=0.80, location=(x * 0.5, y * 0.5, z * 0.5))
        cube = bpy.context.object
        cube.parent = pivot
        bevel = cube.modifiers.new("bevel", "BEVEL")
        bevel.width = 0.022
        bevel.segments = 2
        mat = bpy.data.materials.new("shade")
        mat.use_nodes = True
        bsdf = next(n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED")
        colour = (shade, shade, shade, 1.0)
        bsdf.inputs["Base Color"].default_value = colour
        bsdf.inputs["Roughness"].default_value = 0.42
        # a little light of their own, so the faces turned away from the lamp stay readable
        for name, value in (("Emission Color", colour), ("Emission Strength", 0.22)):
            if name in bsdf.inputs:
                bsdf.inputs[name].default_value = value
        cube.data.materials.append(mat)

    bpy.app.driver_namespace["cube_angle"] = cube_angle
    driver = pivot.driver_add("rotation_euler", 2).driver
    driver.type = "SCRIPTED"
    driver.expression = "cube_angle(frame)"

    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 3.5
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    cam.location = Vector((1.0, -1.0, 0.82)).normalized() * 12
    cam.rotation_euler = (-cam.location).to_track_quat("-Z", "Y").to_euler()
    scene.camera = cam

    def sun(name, direction, strength):
        data = bpy.data.lights.new(name, "SUN")
        data.energy = strength
        data.angle = math.radians(12)
        obj = bpy.data.objects.new(name, data)
        scene.collection.objects.link(obj)
        obj.rotation_euler = Vector(direction).to_track_quat("Z", "Y").to_euler()

    sun("key", (-0.35, -0.75, 1.0), 3.0)   # from above and a little to the left
    sun("fill", (1.0, -0.4, 0.15), 0.5)

    world = bpy.data.worlds.new("world")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.05
    scene.world = world

    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = 96
    scene.cycles.use_denoising = True
    scene.render.film_transparent = True
    scene.render.use_motion_blur = True
    scene.render.motion_blur_shutter = 0.45
    scene.render.resolution_x = scene.render.resolution_y = SIZE
    scene.render.resolution_percentage = 100
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    settings = scene.render.image_settings
    if hasattr(settings, "media_type"):
        settings.media_type = "IMAGE"
    settings.file_format = "PNG"
    settings.color_mode = "RGBA"
    settings.color_depth = "8"

    os.makedirs(OUT, exist_ok=True)
    if "--still" in sys.argv:
        # one large picture of the cube at rest, for the logo and the installer
        scene.render.resolution_x = scene.render.resolution_y = 1024
        scene.render.use_motion_blur = False
        scene.cycles.samples = 256
        scene.frame_set(0)
        scene.render.filepath = os.path.join(OUT, "still.png")
        bpy.ops.render.render(write_still=True)
        print("rendered still to", OUT)
        return
    scene.frame_start, scene.frame_end = 0, FRAMES - 1
    scene.render.filepath = os.path.join(OUT, "f_")
    bpy.ops.render.render(animation=True)
    print("rendered", FRAMES, "frames to", OUT)


main()
