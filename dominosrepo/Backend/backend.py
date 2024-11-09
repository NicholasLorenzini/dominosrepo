from fastapi import FastAPI, UploadFile, File
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse, FileResponse
from fastapi.staticfiles import StaticFiles
from io import BytesIO
from PIL import Image
from ultralytics import YOLO
import logging
import numpy as np
import os
import uuid
import cv2
from collections import defaultdict

# Set up basic logging
logging.basicConfig(level=logging.INFO)

# Load your custom YOLOv11 model
model = YOLO(r"C:\Users\Nick\Desktop\Dominos\dominosrepo\Backend\best.pt", task="detect")

app = FastAPI()

# CORS middleware for allowing all origins (adjust in production)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Define the label mapping and color mapping
label_map = {
    0: 'pip-0', 1: 'pip-1', 2: 'pip-10', 3: 'pip-11', 4: 'pip-12',
    5: 'pip-2', 6: 'pip-3', 7: 'pip-4', 8: 'pip-5', 9: 'pip-6',
    10: 'pip-7', 11: 'pip-8', 12: 'pip-9', 13: 'blank'
}

# Assign a distinct color for each label (BGR format)
color_map = {
    'pip-0': (255, 0, 0),
    'pip-1': (61, 185, 252),
    'pip-10': (222, 144, 75),
    'pip-11': (186, 40, 35),
    'pip-12': (156, 148, 148),
    'pip-2': (52, 217, 55),
    'pip-3': (222, 60, 68),
    'pip-4': (104, 212, 170),
    'pip-5': (63, 61, 191),
    'pip-6': (230, 227, 90),
    'pip-7': (219, 81, 203),
    'pip-8': (17, 156, 5),
    'pip-9': (128, 5, 156),
    'blank': (49, 51, 50)
}

# Define the absolute path to the external img directory
external_img_directory = r"C:\Users\Nick\Desktop\Dominos\dominosrepo\img"
if not os.path.exists(external_img_directory):
    os.makedirs(external_img_directory)

@app.post("/process-image")
async def process_image(file: UploadFile = File(...)):
    logging.info("Received request to /process-image")
    
    # Read the uploaded file
    contents = await file.read()
    logging.info(f"Uploaded file size: {len(contents)} bytes")

    try:
        image = Image.open(BytesIO(contents))
        logging.info("Image successfully opened")

        # Perform YOLOv11 inference on the image
        logging.info("Running YOLO inference...")
        results = model(image)
        logging.info(f"YOLO inference completed with {len(results)} results")

        # Initialize a dictionary to count each label
        label_counts = defaultdict(int)

        # Convert the image to a numpy array for manual drawing
        image_np = np.array(image)

        # Specify your desired line thickness for bounding boxes and font thickness
        line_thickness = 2
        font_scale = 0.9
        font_thickness = 2  # Thicker font for readability

        # Draw bounding boxes manually and build the detections list
        detections = []
        for result in results:
            for detection in result.boxes:
                class_label = int(detection.cls.item())
                x1, y1, x2, y2 = [float(coord) for coord in detection.xyxy[0].tolist()]
                confidence = float(detection.conf)

                label_name = label_map.get(class_label, "Unknown")
                label_counts[label_name] += 1
                color = color_map.get(label_name, (0, 255, 0))  # Default to green if label not in color_map

                detections.append({
                    "box_": [[x1, y1, x2, y2]],
                    "confidence": confidence,
                    "label": label_name
                })

                # Draw the bounding box
                cv2.rectangle(image_np, (int(x1), int(y1)), (int(x2), int(y2)), color=color, thickness=line_thickness)

                # Draw label with a white background and drop shadow
                label_text = f"{label_name} "
                text_x = int(x1)
                text_y = int(y1) - 10 if y1 - 10 > 10 else int(y1) + 20

                (text_width, text_height), _ = cv2.getTextSize(label_text, cv2.FONT_HERSHEY_SIMPLEX, font_scale, font_thickness)
                
                # Draw filled rectangle (background for the text)
                cv2.rectangle(
                    image_np, 
                    (text_x, text_y - text_height - 5), 
                    (text_x + text_width, text_y + 5), 
                    (255, 255, 255),
                    thickness=cv2.FILLED
                )

                # Draw shadow
                shadow_offset = (2, 1)
                shadow_color = (0, 0, 0)
                cv2.putText(
                    image_np, label_text, 
                    (text_x + shadow_offset[0], text_y + shadow_offset[1]), 
                    cv2.FONT_HERSHEY_SIMPLEX, font_scale, shadow_color, font_thickness
                )

                # Draw actual label text
                cv2.putText(
                    image_np, label_text, 
                    (text_x, text_y), 
                    cv2.FONT_HERSHEY_SIMPLEX, font_scale, color, font_thickness
                )

        # Convert the modified numpy array back to a PIL image
        image_pil = Image.fromarray(image_np)

        if image_pil.mode != "RGB":
            image_pil = image_pil.convert("RGB")

        # Save the annotated image
        filename = f"{uuid.uuid4()}.png"
        filepath = os.path.join(external_img_directory, filename)
        image_pil.save(filepath, "PNG")
        logging.info(f"Processed image saved to {filepath}")

        # Prepare the response with URL, detections, and counts
        image_url = f"https://5ecb-70-190-110-222.ngrok-free.app/external-img/{filename}"
        logging.info(f"Generated image URL: {image_url}")
        return JSONResponse(content={
            "url": image_url,
            "detections": detections,
            "counts": label_counts
        })

    except Exception as e:
        logging.error(f"Error processing image: {str(e)}")
        return JSONResponse(content={
            "error": str(e),
            "url": "",
            "detections": [],
            "counts": {}
        })

@app.get("/external-img/{filename}")
async def get_image(filename: str):
    filepath = os.path.join(external_img_directory, filename)
    return FileResponse(filepath, media_type="image/png")

@app.delete("/delete-image")
async def delete_image(filename: str):
    filepath = os.path.join(external_img_directory, filename)
    if os.path.exists(filepath):
        os.remove(filepath)
        return {"message": "Image deleted successfully"}
    else:
        return {"error": "Image not found"}

# Mount the external img directory for static file serving
app.mount("/external-img", StaticFiles(directory=external_img_directory), name="external-img")

# To run the server:
# uvicorn backend:app --reload
# uvicorn backend:app --host 0.0.0.0 --port 8000
#/opt/render/project/src/img
#/opt/render/project/src/img
# 192.168.0.55
# python -m uvicorn backend:app --host 0.0.0.0 --port 8000