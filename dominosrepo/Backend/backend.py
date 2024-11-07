from fastapi import FastAPI, UploadFile, File
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
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
model = YOLO("/mnt/c/Users/Nick/Desktop/Dominos/dominosrepo/Backend/best.pt", task="detect")

app = FastAPI()

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://127.0.0.1:8080"],  # List the allowed origins (your frontend URL)
    allow_credentials=True,
    allow_methods=["*"],  # Allow all methods (POST, GET, etc.)
    allow_headers=["*"],  # Allow all headers
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
    'blank': (49, 51, 50)       # Black
}

# Define the absolute path to the external img directory
external_img_directory = "/mnt/c/Users/Nick/Desktop/Dominos/dominosrepo/img"  # Replace with your absolute path

# Ensure the external img directory exists
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
                # Convert the tensor to an integer index and get the bounding box coordinates
                class_label = int(detection.cls.item())
                x1, y1, x2, y2 = [float(coord) for coord in detection.xyxy[0].tolist()]  # Ensure float list
                confidence = float(detection.conf)

                # Map the label to its name and color, increment the count, and add to detections list
                label_name = label_map.get(class_label, "Unknown")
                label_counts[label_name] += 1
                color = color_map.get(label_name, (0, 255, 0))  # Default to green if label not in color_map

                detections.append({
                    "box_": [[x1, y1, x2, y2]],  # Nested list to match Vec<Vec<f32>>
                    "confidence": confidence,
                    "label": label_name
                })

                # Draw the bounding box
                cv2.rectangle(image_np, (int(x1), int(y1)), (int(x2), int(y2)), color=color, thickness=line_thickness)

                # Draw label with a white background and drop shadow
                label_text = f"{label_name} "
                text_x = int(x1)
                text_y = int(y1) - 10 if y1 - 10 > 10 else int(y1) + 20

                # Get the width and height of the text box
                (text_width, text_height), _ = cv2.getTextSize(label_text, cv2.FONT_HERSHEY_SIMPLEX, font_scale, font_thickness)

                # Draw filled rectangle (background for the text)
                cv2.rectangle(
                    image_np, 
                    (text_x, text_y - text_height - 5), 
                    (text_x + text_width, text_y + 5), 
                    (255, 255, 255),  # White background
                    thickness=cv2.FILLED
                )

                # Draw shadow by offsetting the text slightly and in black
                shadow_offset = (2, 1)  # Offset for the shadow effect
                shadow_color = (0, 0, 0)  # Black for shadow
                cv2.putText(
                    image_np, label_text, 
                    (text_x + shadow_offset[0], text_y + shadow_offset[1]), 
                    cv2.FONT_HERSHEY_SIMPLEX, font_scale, shadow_color, font_thickness
                )

                # Draw actual label text in color on top of the shadow and white background
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
        image_url = f"http://localhost:8000/external-img/{filename}"
        logging.info(f"Generated image URL: {image_url}")  # Debug log for image URL
        return JSONResponse(content={
            "url": image_url,
            "detections": detections,
            "counts": label_counts  # Include label counts in the response
        })

    except Exception as e:
        logging.error(f"Error processing image: {str(e)}")
        # Return a consistent JSON structure with empty fields in case of an error
        return JSONResponse(content={
            "error": str(e),
            "url": "",  # Ensure URL field is present in error case
            "detections": [],
            "counts": {}
        })
    
@app.delete("/delete-image")
async def delete_image(filename: str):
    filepath = os.path.join(external_img_directory, filename)
    if os.path.exists(filepath):
        os.remove(filepath)
        return {"message": "Image deleted successfully"}
    else:
        return {"error": "Image not found"}

# Mount the external img directory to serve static files
app.mount("/external-img", StaticFiles(directory=external_img_directory), name="external-img")


# To run the server:
# uvicorn backend:app --reload
